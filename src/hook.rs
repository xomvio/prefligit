use std::fmt::Display;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::ValueEnum;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use thiserror::Error;
use url::Url;

use crate::config::{
    self, read_config, read_manifest, ConfigLocalHook, ConfigLocalRepo, ConfigRemoteRepo,
    ConfigRepo, ConfigWire, ManifestHook, Stage, CONFIG_FILE, MANIFEST_FILE,
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
    #[error(transparent)]
    Store(#[from] Box<crate::store::Error>),
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
    pub fn local(hooks: Vec<ConfigLocalHook>) -> Result<Self, Error> {
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
    pub fn from_directory(root: PathBuf, config: Option<PathBuf>) -> Result<Self, Error> {
        let config_path = config.unwrap_or_else(|| root.join(CONFIG_FILE));
        let config = read_config(&config_path)?;
        Ok(Self { root, config })
    }

    /// Load project configuration from the current directory.
    pub fn current(config: Option<PathBuf>) -> Result<Self, Error> {
        Self::from_directory(CWD.clone(), config)
    }

    /// Load and prepare hooks for the project.
    pub async fn hooks(&self, store: &Store) -> Result<Vec<Hook>, Error> {
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
            let repo_path = repo_path.map_err(Box::new)?;

            // Read the repo manifest.
            let repo = Repo::remote(
                repo_config.repo.as_str(),
                &repo_config.rev,
                &repo_path.to_string_lossy(),
            )?;

            // Prepare remote hooks.
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

                // Prepare hooks with `additional_dependencies` (they need separate repos).
                if let Some(deps) = hook.additional_dependencies.clone() {
                    let repo_config = repo_config.clone();
                    let repo_source = repo.to_string();
                    hook_tasks.push(async move {
                        let path = store.prepare_remote_repo(&repo_config, Some(deps)).await;
                        (repo_source, hook, path)
                    });
                } else {
                    hooks.push(Hook::new(hook, repo.to_string(), Some(repo_path.clone())));
                }
            }
        }

        while let Some((source, hook, result)) = hook_tasks.next().await {
            let path = result.map_err(Box::new)?;
            hooks.push(Hook::new(hook, source, Some(path)));
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
            if hook.language.need_environment() {
                let path = store
                    .prepare_local_repo(&hook, hook.additional_dependencies.clone())
                    .await
                    .map_err(Box::new)?;
                hooks.push(Hook::new_local(hook, Some(path)));
            } else {
                hooks.push(Hook::new_local(hook, None));
            }
        }

        Ok(hooks)
    }
}

#[derive(Debug)]
pub struct Hook {
    src: String,
    path: Option<PathBuf>,
    config: ManifestHook,
}

impl Display for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            if let Some(ref path) = self.path {
                write!(
                    f,
                    "{} ({} at {})",
                    self.config.id,
                    self.src,
                    path.to_string_lossy()
                )
            } else {
                write!(f, "{} ({})", self.config.id, self.src)
            }
        } else {
            write!(f, "{}", self.config.id)
        }
    }
}

impl Hook {
    /// Create a new hook with a configuration and an optional path.
    /// The path is `None` if the hook doesn't need a environment.
    pub fn new(config: ManifestHook, src: String, path: Option<PathBuf>) -> Self {
        Self { src, config, path }
    }

    pub fn new_local(config: ManifestHook, path: Option<PathBuf>) -> Self {
        Self {
            src: "local".to_string(),
            config,
            path,
        }
    }

    pub fn source(&self) -> &str {
        &self.src
    }

    /// Get the working directory for the hook.
    pub fn path(&self) -> &Path {
        self.path.as_ref().unwrap_or(&CWD)
    }

    pub fn language_version(&self) -> &str {
        self.config
            .language_version
            .as_ref()
            .map_or("default", Deref::deref)
    }

    pub fn id(&self) -> &str {
        &self.config.id
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub fn entry(&self) -> &str {
        &self.config.entry
    }

    pub fn alias(&self) -> Option<&str> {
        self.config.alias.as_deref()
    }

    pub fn files(&self) -> &str {
        self.config.files.as_ref().map_or("", Deref::deref)
    }

    pub fn exclude(&self) -> &str {
        self.config.exclude.as_ref().map_or("^$", Deref::deref)
    }

    pub fn types(&self) -> Vec<&str> {
        self.config
            .types
            .as_ref()
            .map_or_else(|| vec!["file"], |t| t.iter().map(Deref::deref).collect())
    }

    pub fn types_or(&self) -> Option<&Vec<String>> {
        self.config.types.as_ref()
    }

    pub fn exclude_types(&self) -> Vec<&str> {
        self.config
            .exclude_types
            .as_ref()
            .map_or_else(Vec::new, |t| t.iter().map(Deref::deref).collect())
    }

    pub fn additional_dependencies(&self) -> Option<&Vec<String>> {
        self.config.additional_dependencies.as_ref()
    }

    pub fn always_run(&self) -> bool {
        self.config.always_run.unwrap_or(false)
    }

    pub fn fail_fast(&self) -> bool {
        self.config.fail_fast.unwrap_or(false)
    }

    pub fn pass_filenames(&self) -> bool {
        self.config.pass_filenames.unwrap_or(true)
    }

    pub fn description(&self) -> Option<&str> {
        self.config.description.as_deref()
    }

    pub fn log_file(&self) -> Option<&str> {
        self.config.log_file.as_deref()
    }

    pub fn require_serial(&self) -> bool {
        self.config.require_serial.unwrap_or(false)
    }

    pub fn verbose(&self) -> bool {
        self.config.verbose.unwrap_or(false)
    }

    pub fn minimum_pre_commit_version(&self) -> Option<&str> {
        self.config.minimum_pre_commit_version.as_deref()
    }

    pub fn stages(&self) -> &[Stage] {
        self.config
            .stages
            .as_ref()
            .map_or_else(|| Stage::value_variants(), |s| s.as_slice())
    }

    /// Get the environment directory that the hook will be installed to.
    fn environment_dir(&self) -> PathBuf {
        let lang = self.config.language;
        self.path()
            // TODO
            .join(lang.environment_dir().unwrap())
            .join(self.language_version())
    }

    /// Check if the hook is installed.
    pub fn installed(&self) -> bool {
        if self.path.is_none() {
            return true;
        };

        // let lang = self.config.language;
        false
    }
}
