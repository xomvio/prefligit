use std::fmt::Display;
use std::fmt::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::ValueEnum;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use itertools::Itertools;
use thiserror::Error;
use tracing::{debug, error};
use url::Url;

use crate::config::{
    self, read_config, read_manifest, ConfigLocalHook, ConfigLocalRepo, ConfigRemoteHook,
    ConfigRemoteRepo, ConfigRepo, ConfigWire, Language, ManifestHook, Stage, CONFIG_FILE,
    MANIFEST_FILE,
};
use crate::fs::CWD;
use crate::languages::DEFAULT_VERSION;
use crate::printer::Printer;
use crate::store::Store;
use crate::warn_user;

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
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub enum Repo {
    Remote {
        /// Path to the stored repo.
        path: PathBuf,
        url: Url,
        rev: String,
        hooks: Vec<ManifestHook>,
    },
    Local {
        hooks: Vec<ManifestHook>,
    },
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

        Ok(Self::Remote {
            path,
            url,
            rev: rev.to_string(),
            hooks,
        })
    }

    /// Construct a local repo from a list of hooks.
    pub fn local(hooks: Vec<ConfigLocalHook>) -> Result<Self, Error> {
        Ok(Self::Local { hooks })
    }

    pub fn meta() -> Self {
        todo!()
    }

    /// Get a hook by id.
    pub fn get_hook(&self, id: &str) -> Option<&ManifestHook> {
        let hooks = match self {
            Repo::Remote { ref hooks, .. } => hooks,
            Repo::Local { ref hooks } => hooks,
            Repo::Meta => return None,
        };
        hooks.iter().find(|hook| hook.id == id)
    }
}

impl Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Repo::Remote { url, rev, .. } => write!(f, "{}@{}", url, rev),
            Repo::Local { .. } => write!(f, "local"),
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
    pub async fn prepare_hooks(&self, store: &Store, printer: Printer) -> Result<Vec<Hook>, Error> {
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
                        store.prepare_remote_repo(remote_repo, None, printer).await,
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

                let mut builder = HookBuilder::new(repo.to_string(), manifest_hook.clone());
                builder.update(hook_config);
                builder.combine(&self.config);
                let hook = builder.build();

                // Prepare hooks with `additional_dependencies` (they need separate repos).
                if !hook.additional_dependencies.is_empty() {
                    let repo_config = repo_config.clone();
                    let deps = hook.additional_dependencies.clone();

                    hook_tasks.push(async move {
                        let path = store
                            .prepare_remote_repo(&repo_config, Some(deps), printer)
                            .await?;
                        Ok::<Hook, crate::store::Error>(hook.with_path(path))
                    });
                } else {
                    hooks.push(hook.with_path(repo_path.clone()));
                }
            }
        }

        while let Some(result) = hook_tasks.next().await {
            let hook = result.map_err(Box::new)?;
            hooks.push(hook);
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
            let hook = hook_config.clone();

            // If the hook doesn't need an environment, don't do any preparation.
            if hook.language.environment_dir().is_some() {
                let path = store
                    .prepare_local_repo(&hook, hook.additional_dependencies.clone(), printer)
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

struct HookBuilder {
    src: String,
    config: ManifestHook,
}

impl HookBuilder {
    fn new(src: String, config: ManifestHook) -> Self {
        Self { src, config }
    }

    /// Update the hook from the project level hook configuration.
    fn update(&mut self, config: &ConfigRemoteHook) -> &mut Self {
        macro_rules! update_if_some {
            ($($field:ident),* $(,)?) => {
                $(
                if config.$field.is_some() {
                    self.config.$field = config.$field.clone();
                }
                )*
            };
        }
        update_if_some!(
            alias,
            files,
            exclude,
            types,
            types_or,
            exclude_types,
            additional_dependencies,
            args,
            always_run,
            fail_fast,
            pass_filenames,
            description,
            language_version,
            log_file,
            require_serial,
            stages,
            verbose,
            minimum_pre_commit_version,
        );

        if let Some(name) = &config.name {
            self.config.name = name.clone();
        }
        if let Some(entry) = &config.entry {
            self.config.entry = entry.clone();
        }
        if let Some(language) = &config.language {
            self.config.language = language.clone();
        }

        self
    }

    /// Combine the hook configuration with the project level hook configuration.
    fn combine(&mut self, config: &ConfigWire) {
        let language = self.config.language;
        if self.config.language_version.is_none() {
            self.config.language_version = config
                .default_language_version
                .as_ref()
                .and_then(|v| v.get(&language).cloned())
        }
        if self.config.language_version.is_none() {
            self.config.language_version = Some(language.default_version().to_string());
        }

        if self.config.stages.is_none() {
            self.config.stages = config.default_stages.clone();
        }
    }

    /// Fill in the default values for the hook configuration.
    fn fill_in_defaults(&mut self) {
        self.config
            .language_version
            .get_or_insert(DEFAULT_VERSION.to_string());
        self.config.alias.get_or_insert("".to_string());
        self.config.args.get_or_insert(Vec::new());
        self.config.types.get_or_insert(vec!["file".to_string()]);
        self.config.types_or.get_or_insert(Vec::new());
        self.config.exclude_types.get_or_insert(Vec::new());
        self.config.always_run.get_or_insert(false);
        self.config.fail_fast.get_or_insert(false);
        self.config.pass_filenames.get_or_insert(true);
        self.config.require_serial.get_or_insert(false);
        self.config.verbose.get_or_insert(false);
        self.config
            .stages
            .get_or_insert(Stage::value_variants().to_vec());
        self.config
            .additional_dependencies
            .get_or_insert(Vec::new());
    }

    /// Check the hook configuration.
    fn check(&self) {
        let language = self.config.language;
        // TODO: check ENVIRONMENT_DIR with language_version and additional_dependencies
        if language.environment_dir().is_none() {
            if self.config.language_version.is_some() {
                warn_user!(
                    "Language {} does not need environment, but language_version is set",
                    language
                );
            }

            if self.config.additional_dependencies.is_some() {
                warn_user!(
                    "Language {} does not need environment, but additional_dependencies is set",
                    language
                );
            }
        }
    }

    /// Build the hook.
    fn build(mut self) -> Hook {
        self.check();
        self.fill_in_defaults();

        Hook {
            src: self.src,
            path: None,
            id: self.config.id,
            name: self.config.name,
            entry: self.config.entry,
            language: self.config.language,
            alias: self.config.alias.expect("alias not set"),
            files: self.config.files,
            exclude: self.config.exclude,
            types: self.config.types.expect("types not set"),
            types_or: self.config.types_or.expect("types_or not set"),
            exclude_types: self.config.exclude_types.expect("exclude_types not set"),
            additional_dependencies: self
                .config
                .additional_dependencies
                .expect("additional_dependencies should not be None"),
            args: self.config.args.expect("args not set"),
            always_run: self.config.always_run.expect("always_run not set"),
            fail_fast: self.config.fail_fast.expect("fail_fast not set"),
            pass_filenames: self.config.pass_filenames.expect("pass_filenames not set"),
            description: self.config.description,
            language_version: self
                .config
                .language_version
                .expect("language_version not set"),
            log_file: self.config.log_file,
            require_serial: self.config.require_serial.expect("require_serial not set"),
            stages: self.config.stages.expect("stages not set"),
            verbose: self.config.verbose.expect("verbose not set"),
            minimum_pre_commit_version: self.config.minimum_pre_commit_version,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Hook {
    src: String,
    path: Option<PathBuf>,

    pub id: String,
    pub name: String,
    pub entry: String,
    pub language: Language,
    pub alias: String,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub types: Vec<String>,
    pub types_or: Vec<String>,
    pub exclude_types: Vec<String>,
    pub additional_dependencies: Vec<String>,
    pub args: Vec<String>,
    pub always_run: bool,
    pub fail_fast: bool,
    pub pass_filenames: bool,
    pub description: Option<String>,
    pub language_version: String,
    pub log_file: Option<String>,
    pub require_serial: bool,
    pub stages: Vec<Stage>,
    pub verbose: bool,
    pub minimum_pre_commit_version: Option<String>,
}

impl Display for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            if let Some(ref path) = self.path {
                write!(
                    f,
                    "{} ({} at {})",
                    self.id,
                    self.src,
                    path.to_string_lossy()
                )
            } else {
                write!(f, "{} ({})", self.id, self.src)
            }
        } else {
            write!(f, "{}", self.id)
        }
    }
}

impl Hook {
    /// Create a local hook.
    pub fn new_local(config: ManifestHook, path: Option<PathBuf>) -> Self {
        let builder = HookBuilder::new("local".to_string(), config);
        let mut hook = builder.build();
        hook.path = path;
        hook
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn source(&self) -> &str {
        &self.src
    }

    /// Get the working directory for the hook.
    pub fn path(&self) -> &Path {
        self.path.as_ref().unwrap_or_else(|| CWD.deref())
    }

    /// Get the environment directory that the hook will be installed to.
    pub fn environment_dir(&self) -> Option<PathBuf> {
        let lang = self.language;
        let Some(env_dir) = lang.environment_dir() else {
            return None;
        };
        Some(
            self.path()
                .join(format!("{}-{}", env_dir, &self.language_version)),
        )
    }

    pub fn install_key(&self) -> String {
        format!(
            "{}-{}-{}-{}",
            self.src,
            self.language.to_string(),
            self.language_version,
            self.additional_dependencies.join(",")
        )
    }

    // TODO: health check
    /// Check if the hook is installed in the environment.
    pub fn installed(&self) -> bool {
        let Some(env) = self.environment_dir() else {
            return true;
        };

        let state_file_v2 = env.join(".install_state_v2");
        let state_file_v1 = env.join(".install_state_v1");

        if state_file_v2.exists() {
            return true;
        };

        let state_v1 = match fs_err::read_to_string(&state_file_v1) {
            Ok(state) => state,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return false,
            Err(err) => {
                error!("Failed to read install state file: {}", err);
                return false;
            }
        };

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        struct StateV1 {
            additional_dependencies: Vec<String>,
        }
        let state_v1: StateV1 = match serde_json::from_str(&state_v1) {
            Ok(state) => state,
            Err(err) => {
                error!("Failed to parse install state file: {}", err);
                return false;
            }
        };

        state_v1.additional_dependencies == self.additional_dependencies
    }

    /// Write a state file to mark the hook as installed.
    pub fn mark_installed(&self) -> Result<(), Error> {
        let env = self.environment_dir().unwrap();
        let state_file_v2 = env.join(".install_state_v2");
        fs_err::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&state_file_v2)?;
        Ok(())
    }
}

async fn install_hook(hook: &Hook, env_dir: PathBuf, printer: Printer) -> Result<()> {
    writeln!(
        printer.stdout(),
        "Installing environment for {}",
        hook.source()
    )?;

    if env_dir.try_exists()? {
        debug!(
            "Removing existing environment directory {}",
            env_dir.display()
        );
        fs_err::remove_dir_all(&env_dir)?;
    }

    hook.language.install(hook).await?;
    hook.mark_installed()?;

    Ok(())
}

// TODO: progress bar
pub async fn install_hooks(hooks: &[Hook], printer: Printer) -> Result<()> {
    let to_install = hooks
        .iter()
        .filter(|&hook| !hook.installed())
        .unique_by(|&hook| hook.install_key());

    let mut tasks = FuturesUnordered::new();
    for hook in to_install {
        if let Some(env_dir) = hook.environment_dir() {
            tasks.push(async move { install_hook(hook, env_dir, printer).await });
        }
    }
    while let Some(result) = tasks.next().await {
        result?;
    }

    Ok(())
}

async fn run_hook(hook: &Hook, filenames: Vec<String>, printer: Printer) -> Result<()> {
    // TODO: check files diff
    // TODO: group filenames and run in parallel

    writeln!(printer.stdout(), "Running hook {}", hook)?;
    let start = std::time::Instant::now();
    hook.language.run(hook, filenames).await?;
    writeln!(
        printer.stdout(),
        "{} completed in {:?}",
        hook,
        start.elapsed()
    )?;

    Ok(())
}

pub async fn run_hooks(hooks: &[Hook], skips: &[String], printer: Printer) -> Result<()> {
    // TODO: collect files
    // TODO: classify files

    // hooks must run in serial
    for hook in hooks {
        if skips.contains(&hook.id) || skips.contains(&hook.alias) {
            writeln!(printer.stdout(), "Skipping hook `{}`", hook)?;
            continue;
        }

        // TODO: handle single hook result
        run_hook(hook, vec![], printer).await?
    }

    Ok(())
}
