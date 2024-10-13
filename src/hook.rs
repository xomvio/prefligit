use std::path::PathBuf;

use anyhow::Result;
use thiserror::Error;

use crate::config::{
    self, read_config, ConfigWire, HookWire, Language, ManifestHook, RepoLocation, Stage,
    CONFIG_FILE,
};
use crate::fs::CWD;
use crate::store::{Repo, Store};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to read config file: {0}")]
    ReadConfig(#[from] config::Error),
    #[error("Failed to initialize repo: {0}")]
    InitRepo(#[from] anyhow::Error),
    #[error("Hook not found: {hook} in repo {repo}")]
    HookNotFound { hook: String, repo: RepoLocation },
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

    pub fn repos(&self, store: &Store) -> Result<Vec<Repo>> {
        // TODO: init in parallel
        self.config
            .repos
            .iter()
            .map(|repo| store.init_repo(repo, None))
            .collect::<Result<_>>()
    }

    pub fn hooks(&self, store: &Store) -> Result<Vec<Hook>> {
        let mut hooks = Vec::new();

        for repo_config in &self.config.repos {
            let repo = store.init_repo(repo_config, None)?;

            for hook_config in &repo_config.hooks {
                let Some(store_hook) = repo.hooks().get(&hook_config.id) else {
                    // Check hook id is valid.
                    return Err(Error::HookNotFound {
                        hook: hook_config.id.clone(),
                        repo: repo_config.repo.clone(),
                    })?;
                };

                // TODO: avoid clone
                let mut hook = Hook::new(store_hook.clone());
                hook.update(hook_config);
                hook.fill(&self.config);

                hooks.push(hook);
            }
        }
        Ok(hooks)
    }
}

#[derive(Debug)]
pub struct Hook {
    // Basic hook fields from the manifest.
    pub id: String,
    pub name: String,
    pub entry: String,
    pub language: Language,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub types: Option<Vec<String>>,
    pub types_or: Option<Vec<String>>,
    pub exclude_types: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub fail_fast: Option<bool>,
    pub verbose: Option<bool>,
    pub pass_filenames: Option<bool>,
    pub require_serial: Option<bool>,
    pub description: Option<String>,
    pub language_version: Option<String>,
    pub minimum_pre_commit_version: Option<String>,
    pub args: Option<Vec<String>>,
    pub stages: Option<Vec<Stage>>,

    // Additional fields from the repo configuration.
    pub alias: Option<String>,
    pub additional_dependencies: Option<Vec<String>>,
    pub log_file: Option<String>,
    // repo: Repo,
}

impl Hook {
    pub fn new(hook: ManifestHook) -> Self {
        Self {
            // repo,
            id: hook.id,
            name: hook.name,
            entry: hook.entry,
            language: hook.language,
            files: hook.files,
            exclude: hook.exclude,
            types: hook.types,
            types_or: hook.types_or,
            exclude_types: hook.exclude_types,
            always_run: hook.always_run,
            fail_fast: hook.fail_fast,
            verbose: hook.verbose,
            pass_filenames: hook.pass_filenames,
            require_serial: hook.require_serial,
            description: hook.description,
            language_version: hook.language_version,
            minimum_pre_commit_version: hook.minimum_pre_commit_version,
            args: hook.args,
            stages: hook.stages,

            additional_dependencies: None,
            log_file: None,
            alias: None,
        }
    }

    pub fn update(&mut self, repo_hook: &HookWire) {
        self.alias = repo_hook.alias.clone();

        if let Some(name) = &repo_hook.name {
            self.name = name.clone();
        }
        if let Some(language_version) = &repo_hook.language_version {
            self.language_version = Some(language_version.clone());
        }
        if let Some(files) = &repo_hook.files {
            self.files = Some(files.clone());
        }
        if let Some(exclude) = &repo_hook.exclude {
            self.exclude = Some(exclude.clone());
        }
        if let Some(types) = &repo_hook.types {
            self.types = Some(types.clone());
        }
        if let Some(types_or) = &repo_hook.types_or {
            self.types_or = Some(types_or.clone());
        }
        if let Some(exclude_types) = &repo_hook.exclude_types {
            self.exclude_types = Some(exclude_types.clone());
        }
        if let Some(args) = &repo_hook.args {
            self.args = Some(args.clone());
        }
        if let Some(stages) = &repo_hook.stages {
            self.stages = Some(stages.clone());
        }
        if let Some(additional_dependencies) = &repo_hook.additional_dependencies {
            self.additional_dependencies = Some(additional_dependencies.clone());
        }
        if let Some(always_run) = &repo_hook.always_run {
            self.always_run = Some(*always_run);
        }
        if let Some(verbose) = &repo_hook.verbose {
            self.verbose = Some(*verbose);
        }
        if let Some(log_file) = &repo_hook.log_file {
            self.log_file = Some(log_file.clone());
        }
    }

    pub fn fill(&mut self, config: &ConfigWire) {
        let language = self.language;
        if self.language_version.is_none() {
            self.language_version = config
                .default_language_version
                .as_ref()
                .and_then(|v| v.get(&language).cloned())
        }
        if self.language_version.is_none() {
            self.language_version = Some(language.default_version());
        }

        if self.stages.is_none() {
            self.stages = config.default_stages.clone();
        }

        // TODO: check ENVIRONMENT_DIR with language_version and additional_dependencies
    }
}
