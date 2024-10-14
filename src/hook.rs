use std::collections::HashMap;
use std::fmt::Display;
use std::future::Future;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use anyhow::Result;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use thiserror::Error;
use url::Url;

use crate::config::{
    self, read_config, read_manifest, ConfigLocalHook, ConfigLocalRepo, ConfigRemoteRepo,
    ConfigRepo, ConfigWire, ManifestHook, CONFIG_FILE, MANIFEST_FILE,
};
use crate::fs::CWD;
use crate::store;
use crate::store::Store;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to parse URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("Failed to read config file: {0}")]
    ReadConfig(#[from] config::Error),
    #[error("Failed to initialize repo: {0}")]
    InitRepo(#[from] anyhow::Error),
    #[error("Hook not found: {hook} in repo {repo}")]
    HookNotFound { hook: String, repo: String },
}

#[derive(Debug, Clone)]
pub struct RemoteRepo {
    /// Path to the stored repo.
    path: PathBuf,
    url: Url,
    rev: String,
    hooks: HashMap<String, ManifestHook>,
}

#[derive(Debug, Clone)]
pub struct LocalRepo {
    hooks: HashMap<String, ConfigLocalHook>,
}

#[derive(Debug, Clone)]
pub enum Repo {
    Remote(RemoteRepo),
    Local(LocalRepo),
    Meta,
}

impl Repo {
    pub fn remote(url: &str, rev: &str, path: &str) -> Result<Self> {
        let url = Url::parse(&url).map_err(Error::InvalidUrl)?;

        let path = PathBuf::from(path);
        let path = path.join(MANIFEST_FILE);
        let manifest = read_manifest(&path)?;
        let hooks = manifest
            .hooks
            .into_iter()
            .map(|hook| (hook.id.clone(), hook))
            .collect();

        Ok(Self::Remote(RemoteRepo {
            path,
            url,
            rev: rev.to_string(),
            hooks,
        }))
    }

    pub fn local(hooks: Vec<ConfigLocalHook>) -> Result<Self> {
        let hooks = hooks
            .into_iter()
            .map(|hook| (hook.id.clone(), hook))
            .collect();

        Ok(Self::Local(LocalRepo { hooks }))
    }

    pub fn meta() -> Self {
        todo!()
    }

    pub fn get_hook(&self, id: &str) -> Option<&ManifestHook> {
        match self {
            Repo::Remote(repo) => repo.hooks.get(id),
            Repo::Local(repo) => repo.hooks.get(id),
            Repo::Meta => None,
        }
    }
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Repo::Remote(repo) => write!(f, "{}@{}", repo.url, repo.rev),
            Repo::Local(_) => write!(f, "local"),
            Repo::Meta => write!(f, "meta"),
        }
    }
}

pub struct Project {
    root: PathBuf,
    config: ConfigWire,
}

impl Project {
    pub fn from_directory(root: PathBuf, config: Option<PathBuf>) -> Result<Self> {
        let config_path = config.unwrap_or_else(|| root.join(CONFIG_FILE));
        let config = read_config(&config_path).map_err(Error::ReadConfig)?;
        Ok(Self { root, config })
    }

    pub fn current(config: Option<PathBuf>) -> Result<Self> {
        Self::from_directory(CWD.clone(), config)
    }

    // pub fn repos(&self, store: &Store) -> Result<Vec<Repo>> {
    //     // TODO: init in parallel
    //     self.config
    //         .repos
    //         .iter()
    //         .map(|repo| store.clone_repo(repo, None))
    //         .collect::<Result<_>>()
    // }

    pub async fn hooks(&self, store: &Store) -> Result<Vec<Hook>> {
        // TODO: progress bar
        // Prepare remote repos.
        let mut tasks = FuturesUnordered::new();
        for repo_config in &self.config.repos {
            if let ConfigRepo::Remote(remote_repo @ ConfigRemoteRepo { .. }) = repo_config {
                tasks.push(async {
                    (
                        remote_repo,
                        store.prepare_remote_repo(remote_repo, None).await,
                    )
                });
            }
        }

        let mut hook_tasks = FuturesUnordered::new();

        while let Some((repo_config, repo_path)) = tasks.next().await {
            let repo_path = repo_path?;

            // Read the repo manifest.
            let repo = Repo::remote(
                repo_config.repo.as_str(),
                &repo_config.rev,
                &repo_path.to_string_lossy(),
            )?;

            for hook_config in &repo_config.hooks {
                // Check hook id is valid.
                let Some(manifest_hook) = repo.get_hook(&hook_config.id) else {
                    return Err(Error::HookNotFound {
                        hook: hook_config.id.clone(),
                        repo: repo.to_string(),
                    }
                    .into());
                };

                let mut hook = manifest_hook.clone();
                hook.update(hook_config.clone());
                hook.fill(&self.config);

                let hook_task: Pin<
                    Box<dyn Future<Output = (ManifestHook, Result<PathBuf>)> + Send>,
                > = if let Some(deps) = &hook.additional_dependencies {
                    Box::pin(async move {
                        (
                            hook,
                            store
                                .prepare_remote_repo(repo_config, Some(deps.clone()))
                                .await,
                        )
                    })
                } else {
                    Box::pin(async move { (hook, Ok(repo_path.clone())) })
                };

                hook_tasks.push(hook_task);
            }
        }

        // Prepare local hooks.
        let local_hooks = self
            .config
            .repos
            .iter()
            .filter_map(|repo| {
                if let ConfigRepo::Local(local_repo @ ConfigLocalRepo { .. }) = repo {
                    Some(local_repo.hooks.clone())
                } else {
                    None
                }
            })
            .flatten();
        for hook_config in local_hooks {
            let mut hook = hook_config.clone();
            hook.fill(&self.config);

            let hook_task: Pin<Box<dyn Future<Output = (ManifestHook, Result<PathBuf>)> + Send>> =
                if hook.language.need_env() {
                    Box::pin(async move {
                        (
                            hook,
                            store
                                .prepare_local_repo(&hook, hook.additional_dependencies.clone())
                                .await,
                        )
                    })
                } else {
                    Box::pin(async move {
                        let err = store::Error::LocalHookNoNeedEnv(hook.id.clone());
                        (hook, Err(err.into()))
                    })
                };

            hook_tasks.push(hook_task);
        }

        // Prepare hooks with `additional_dependencies` (they need separate repos).
        let mut hooks = Vec::new();
        while let Some((hook, repo_result)) = hook_tasks.next().await {
            let path = match repo_result {
                Ok(path) => Some(path),
                Err(err) => match err.downcast_ref::<store::Error>() {
                    Some(store::Error::LocalHookNoNeedEnv(_)) => None,
                    _ => return Err(err),
                },
            };
            hooks.push(Hook::new(hook, path));
        }

        Ok(hooks)
    }
}

#[derive(Debug)]
pub struct Hook {
    config: ManifestHook,
    path: Option<PathBuf>,
}

impl Hook {
    pub fn new(config: ManifestHook, path: Option<PathBuf>) -> Self {
        Self { config, path }
    }

    pub fn path(&self) -> &Path {
        self.path.as_ref().unwrap_or(&CWD)
    }
}

impl Deref for Hook {
    type Target = ManifestHook;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}
