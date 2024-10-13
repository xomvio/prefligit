use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;
use std::path::PathBuf;

use anyhow::Result;
use thiserror::Error;
use url::Url;

use crate::config::{
    self, read_config, read_manifest, ConfigLocalHook, ConfigRemoteHook, ConfigRepo, ConfigWire,
    ManifestHook, CONFIG_FILE, MANIFEST_FILE,
};
use crate::fs::CWD;
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

#[derive(Debug)]
pub struct RemoteRepo {
    /// Path to the stored repo.
    path: PathBuf,
    url: Url,
    rev: String,
    hooks: HashMap<String, ManifestHook>,
}

#[derive(Debug)]
pub enum Repo {
    Remote(RemoteRepo),
    Local(HashMap<String, ConfigLocalHook>),
    Meta,
}

impl Repo {
    pub fn remote(url: String, rev: String, path: String) -> Result<Self> {
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
            rev,
            hooks,
        }))
    }

    pub fn local(hooks: Vec<ConfigLocalHook>) -> Result<Self> {
        let hooks = hooks
            .into_iter()
            .map(|hook| (hook.id.clone(), hook))
            .collect();

        Ok(Self::Local(hooks))
    }

    pub fn meta() -> Self {
        todo!()
    }

    pub fn get_hook(&self, id: &str) -> Option<&ManifestHook> {
        match self {
            Repo::Remote(repo) => repo.hooks.get(id),
            Repo::Local(hooks) => hooks.get(id),
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

    pub fn hooks(&self, store: &Store) -> Result<Vec<Hook>> {
        let mut hooks = Vec::new();

        for repo_config in &self.config.repos {
            match repo_config {
                ConfigRepo::Remote(repo_config) => {
                    let repo = store.clone_repo(repo_config, None)?;

                    for hook_config in &repo_config.hooks {
                        let Some(manifest_hook) = repo.get_hook(&hook_config.id) else {
                            // Check hook id is valid.
                            return Err(Error::HookNotFound {
                                hook: hook_config.id.clone(),
                                repo: repo.to_string(),
                            })?;
                        };

                        let mut hook = Hook::from(manifest_hook.clone());
                        hook.update(hook_config);
                        hook.fill(&self.config);
                        hooks.push(hook);
                    }
                }
                ConfigRepo::Local(repo_config) => {
                    for hook_config in &repo_config.hooks {
                        let mut hook = Hook::from(hook_config.clone());
                        hook.fill(&self.config);
                        hooks.push(hook);
                    }
                }
                ConfigRepo::Meta(_) => {}
            }
        }

        Ok(hooks)
    }
}

#[derive(Debug)]
pub struct Hook(ManifestHook);

impl From<ManifestHook> for Hook {
    fn from(hook: ManifestHook) -> Self {
        Self(hook)
    }
}

impl Deref for Hook {
    type Target = ManifestHook;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hook {
    pub fn update(&mut self, repo_hook: &ConfigRemoteHook) {
        self.0.alias = repo_hook.alias.clone();

        if let Some(name) = &repo_hook.name {
            self.0.name = name.clone();
        }
        if let Some(language_version) = &repo_hook.language_version {
            self.0.language_version = Some(language_version.clone());
        }
        if let Some(files) = &repo_hook.files {
            self.0.files = Some(files.clone());
        }
        if let Some(exclude) = &repo_hook.exclude {
            self.0.exclude = Some(exclude.clone());
        }
        if let Some(types) = &repo_hook.types {
            self.0.types = Some(types.clone());
        }
        if let Some(types_or) = &repo_hook.types_or {
            self.0.types_or = Some(types_or.clone());
        }
        if let Some(exclude_types) = &repo_hook.exclude_types {
            self.0.exclude_types = Some(exclude_types.clone());
        }
        if let Some(args) = &repo_hook.args {
            self.0.args = Some(args.clone());
        }
        if let Some(stages) = &repo_hook.stages {
            self.0.stages = Some(stages.clone());
        }
        if let Some(additional_dependencies) = &repo_hook.additional_dependencies {
            self.0.additional_dependencies = Some(additional_dependencies.clone());
        }
        if let Some(always_run) = &repo_hook.always_run {
            self.0.always_run = Some(*always_run);
        }
        if let Some(verbose) = &repo_hook.verbose {
            self.0.verbose = Some(*verbose);
        }
        if let Some(log_file) = &repo_hook.log_file {
            self.0.log_file = Some(log_file.clone());
        }
    }

    pub fn fill(&mut self, config: &ConfigWire) {
        let language = self.0.language;
        if self.0.language_version.is_none() {
            self.0.language_version = config
                .default_language_version
                .as_ref()
                .and_then(|v| v.get(&language).cloned())
        }
        if self.0.language_version.is_none() {
            self.0.language_version = Some(language.default_version());
        }

        if self.0.stages.is_none() {
            self.0.stages = config.default_stages.clone();
        }

        // TODO: check ENVIRONMENT_DIR with language_version and additional_dependencies
    }
}
