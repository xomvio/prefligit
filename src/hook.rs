use std::fmt::Display;
use std::ops::Deref;
use std::path::{Path, PathBuf};

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
use crate::store::Store;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to parse URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error(transparent)]
    ReadConfig(#[from] config::Error),
    #[error("Hook not found: {hook} in repo {repo}")]
    HookNotFound { hook: String, repo: String },
}

#[derive(Debug, Clone)]
pub struct RemoteRepo {
    /// Path to the stored repo.
    path: PathBuf,
    url: Url,
    rev: String,
    hooks: Vec<ManifestHook>,
}

#[derive(Debug, Clone)]
pub struct LocalRepo {
    hooks: Vec<ManifestHook>,
}

#[derive(Debug, Clone)]
pub enum Repo {
    Remote(RemoteRepo),
    Local(LocalRepo),
    Meta,
}

impl Repo {
    /// Load the remote repo manifest from the path.
    pub fn remote(url: &str, rev: &str, path: &str) -> Result<Self, Error> {
        let url = Url::parse(&url)?;

        let path = PathBuf::from(path);
        let path = path.join(MANIFEST_FILE);
        let manifest = read_manifest(&path)?;
        let hooks = manifest.hooks;

        Ok(Self::Remote(RemoteRepo {
            path,
            url,
            rev: rev.to_string(),
            hooks,
        }))
    }

    /// Construct a local repo from a list of hooks.
    pub fn local(hooks: Vec<ConfigLocalHook>) -> Result<Self> {
        Ok(Self::Local(LocalRepo { hooks }))
    }

    pub fn meta() -> Self {
        todo!()
    }

    /// Get a hook by id.
    pub fn get_hook(&self, id: &str) -> Option<&ManifestHook> {
        let hooks = match self {
            Repo::Remote(repo) => &repo.hooks,
            Repo::Local(repo) => &repo.hooks,
            Repo::Meta => return None,
        };
        hooks.iter().find(|hook| hook.id == id)
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
    /// Load a project configuration from a directory.
    pub fn from_directory(root: PathBuf, config: Option<PathBuf>) -> Result<Self> {
        let config_path = config.unwrap_or_else(|| root.join(CONFIG_FILE));
        let config = read_config(&config_path)?;
        Ok(Self { root, config })
    }

    /// Load project configuration from the current directory.
    pub fn current(config: Option<PathBuf>) -> Result<Self> {
        Self::from_directory(CWD.clone(), config)
    }

    /// Load and prepare hooks for the project.
    pub async fn hooks(&self, store: &Store) -> Result<Vec<Hook>> {
        let mut hooks = Vec::new();

        // TODO: progress bar
        // Prepare remote repos.
        let mut tasks = FuturesUnordered::new();
        let mut hook_tasks = FuturesUnordered::new();

        for repo_config in &self.config.repos {
            if let ConfigRepo::Remote(remote_repo @ ConfigRemoteRepo { .. }) = repo_config {
                tasks.push(async {
                    (
                        remote_repo.clone(),
                        store.prepare_remote_repo(remote_repo, None).await,
                    )
                });
            }
        }

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

                if let Some(deps) = hook.additional_dependencies.clone() {
                    let repo_config = repo_config.clone();
                    hook_tasks.push(async move {
                        let path = store.prepare_remote_repo(&repo_config, Some(deps)).await;
                        (hook, path)
                    });
                } else {
                    hooks.push(Hook::new(hook, Some(repo_path.clone())));
                }
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

            // If the hook doesn't need an environment, don't do any preparation.
            if hook.language.need_env() {
                let path = store
                    .prepare_local_repo(&hook, hook.additional_dependencies.clone())
                    .await?;
                hooks.push(Hook::new(hook, Some(path)));
            } else {
                hooks.push(Hook::new(hook, None));
            }
        }

        // Prepare hooks with `additional_dependencies` (they need separate repos).
        while let Some((hook, repo_result)) = hook_tasks.next().await {
            let path = repo_result?;
            hooks.push(Hook::new(hook, Some(path)));
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
