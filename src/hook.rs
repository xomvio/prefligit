use std::cmp::max;
use std::fmt::Display;
use std::fmt::Write as _;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use anyhow::Result;
use clap::ValueEnum;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use itertools::{zip_eq, Itertools};
use owo_colors::{OwoColorize, Style};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use regex::Regex;
use thiserror::Error;
use tokio::process::Command;
use tracing::{debug, error};
use url::Url;

use crate::config::{
    self, read_config, read_manifest, ConfigLocalHook, ConfigRemoteHook, ConfigRepo, ConfigWire,
    Language, ManifestHook, Stage, CONFIG_FILE, MANIFEST_FILE,
};
use crate::fs::CWD;
use crate::git::get_diff;
use crate::identify::tags_from_path;
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
    #[error("Hook {hook} in not present in repository {repo}")]
    HookNotFound { hook: String, repo: String },
    #[error(transparent)]
    Store(#[from] Box<crate::store::Error>),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Regex(#[from] regex::Error),
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
        let manifest = read_manifest(&path.join(MANIFEST_FILE))?;
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

    pub fn meta() -> Result<Self, Error> {
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

    pub fn path(&self) -> &Path {
        match self {
            Repo::Remote { ref path, .. } => path,
            Repo::Local { .. } => CWD.deref(),
            Repo::Meta => todo!(),
        }
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
    repos: Vec<Rc<Repo>>,
}

impl Project {
    /// Load a project configuration from a directory.
    pub fn from_directory(root: PathBuf, config: Option<PathBuf>) -> Result<Self, Error> {
        let config_path = config.unwrap_or_else(|| root.join(CONFIG_FILE));
        debug!(
            "Loading project configuration from {}",
            config_path.display()
        );
        let config = read_config(&config_path)?;
        let size = config.repos.len();
        Ok(Self {
            root,
            config,
            repos: Vec::with_capacity(size),
        })
    }

    /// Load project configuration from the current directory.
    pub fn current(config: Option<PathBuf>) -> Result<Self, Error> {
        Self::from_directory(CWD.clone(), config)
    }

    pub fn config(&self) -> &ConfigWire {
        &self.config
    }

    async fn init_repos(&mut self, store: &Store, printer: Printer) -> Result<(), Error> {
        let mut repos = Vec::with_capacity(self.config.repos.len());

        // TODO: progress bar
        let mut tasks = FuturesUnordered::new();
        for (idx, repo) in self.config.repos.iter().enumerate() {
            match repo {
                ConfigRepo::Remote(repo) => {
                    tasks.push(async move {
                        let path = store.prepare_remote_repo(repo, &[], printer).await;
                        (idx, path)
                    });
                }
                ConfigRepo::Local(repo) => {
                    let repo = Repo::local(repo.hooks.clone())?;
                    repos.push((idx, Rc::new(repo)));
                }
                ConfigRepo::Meta(_) => {
                    todo!()
                }
            }
        }

        while let Some((idx, repo_path)) = tasks.next().await {
            let repo_path = repo_path.map_err(Box::new)?;
            let ConfigRepo::Remote(repo_config) = &self.config.repos[idx] else {
                unreachable!();
            };
            let repo = Repo::remote(
                repo_config.repo.as_str(),
                &repo_config.rev,
                &repo_path.to_string_lossy(),
            )?;
            repos.push((idx, Rc::new(repo)));
        }

        repos.sort_unstable_by_key(|(idx, _)| *idx);
        self.repos = repos.into_iter().map(|(_, repo)| repo).collect();

        Ok(())
    }

    /// Load and prepare hooks for the project.
    pub async fn init_hooks(
        &mut self,
        store: &Store,
        printer: Printer,
    ) -> Result<Vec<Hook>, Error> {
        self.init_repos(store, printer).await?;

        let mut hooks = Vec::new();

        // TODO: progress bar
        for (repo_config, repo) in zip_eq(self.config.repos.iter(), self.repos.iter()) {
            match repo_config {
                ConfigRepo::Remote(repo_config) => {
                    for hook_config in &repo_config.hooks {
                        // Check hook id is valid.
                        let Some(hook) = repo.get_hook(&hook_config.id) else {
                            return Err(Error::HookNotFound {
                                hook: hook_config.id.clone(),
                                repo: repo.to_string(),
                            }
                            .into());
                        };

                        let repo = Rc::clone(repo);
                        let mut builder = HookBuilder::new(repo, hook.clone());
                        builder.update(hook_config);
                        builder.combine(&self.config);
                        let mut hook = builder.build();

                        // Prepare hooks with `additional_dependencies` (they need separate environments).
                        if !hook.additional_dependencies.is_empty() {
                            let path = store
                                .prepare_remote_repo(
                                    &repo_config,
                                    &hook.additional_dependencies,
                                    printer,
                                )
                                .await
                                .map_err(Box::new)?;

                            hook = hook.with_path(path);
                        } else {
                            // Use the shared repo environment.
                            let path = hook.repo.path().to_path_buf();
                            hook = hook.with_path(path);
                        }

                        hooks.push(hook);
                    }
                }
                ConfigRepo::Local(repo_config) => {
                    for hook_config in &repo_config.hooks {
                        let repo = Rc::clone(repo);
                        let mut builder = HookBuilder::new(repo, hook_config.clone());
                        builder.combine(&self.config);
                        let mut hook = builder.build();

                        // If the hook doesn't need an environment, don't do any preparation.
                        if hook.language.environment_dir().is_some() {
                            let path = store
                                .prepare_local_repo(&hook, &hook.additional_dependencies, printer)
                                .await
                                .map_err(Box::new)?;

                            hook = hook.with_path(path);
                        } else {
                            // Use the shared repo environment.
                            let path = hook.repo.path().to_path_buf();
                            hook = hook.with_path(path);
                        }
                        hooks.push(hook);
                    }
                }
                ConfigRepo::Meta(_) => {
                    todo!()
                }
            }
        }

        Ok(hooks)
    }
}

struct HookBuilder {
    repo: Rc<Repo>,
    config: ManifestHook,
}

impl HookBuilder {
    fn new(repo: Rc<Repo>, config: ManifestHook) -> Self {
        Self { repo, config }
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
            if self.config.language_version != Some(DEFAULT_VERSION.to_string()) {
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
            repo: self.repo,
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
    repo: Rc<Repo>,
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
                    self.repo,
                    path.to_string_lossy()
                )
            } else {
                write!(f, "{} ({})", self.id, self.repo)
            }
        } else {
            write!(f, "{}", self.id)
        }
    }
}

impl Hook {
    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    /// Get the working directory for the hook.
    pub fn path(&self) -> &Path {
        self.path.as_deref().unwrap_or_else(|| self.repo.path())
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
            self.repo,
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
        hook.repo(),
    )?;
    debug!("Install environment for {} to {}", hook, env_dir.display());

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

struct FileTypeFilter {
    all: Vec<String>,
    any: Vec<String>,
    exclude: Vec<String>,
}

impl FileTypeFilter {
    fn new(types: &[String], types_or: &[String], exclude_types: &[String]) -> Self {
        let all = types.to_vec();
        let any = types_or.to_vec();
        let exclude = exclude_types.to_vec();
        Self { all, any, exclude }
    }

    fn filter(&self, file_types: &[&str]) -> bool {
        if !self.all.is_empty() && !self.all.iter().all(|t| file_types.contains(&t.as_str())) {
            return false;
        }
        if !self.any.is_empty() && !self.any.iter().any(|t| file_types.contains(&t.as_str())) {
            return false;
        }
        if self
            .exclude
            .iter()
            .any(|t| file_types.contains(&t.as_str()))
        {
            return false;
        }
        true
    }

    fn from_hook(hook: &Hook) -> Self {
        Self::new(&hook.types, &hook.types_or, &hook.exclude_types)
    }
}

const SKIPPED: &str = "Skipped";
const NO_FILES: &str = "(no files to check)";

fn line(start: &str, cols: usize, end_msg: &str, end_color: Style, postfix: &str) -> String {
    let dots = cols - start.len() - end_msg.len() - postfix.len() - 1;
    format!(
        "{}{}{}{}",
        start,
        ".".repeat(dots),
        postfix,
        end_msg.style(end_color)
    )
}

async fn run_hook(
    hook: &Hook,
    filenames: &[&String],
    skips: &[String],
    diff: Vec<u8>,
    cols: usize,
    verbose: bool,
    printer: Printer,
) -> Result<(bool, Vec<u8>)> {
    // TODO: check files diff
    // TODO: group filenames and run in parallel

    if skips.contains(&hook.id) || skips.contains(&hook.alias) {
        writeln!(
            printer.stdout(),
            "{}",
            line(&hook.name, cols, SKIPPED, Style::new().yellow(), "")
        )?;
        return Ok((true, diff));
    }

    let filenames = filter_filenames(
        filenames.into_par_iter().copied(),
        hook.files.as_deref(),
        hook.exclude.as_deref(),
    )?;

    // TODO: rayon, classify files in parallel
    let filter = FileTypeFilter::from_hook(hook);
    let filenames: Vec<_> = filenames
        .filter(|&filename| {
            let path = Path::new(filename);
            // TODO: log error?
            let file_tags = tags_from_path(path).unwrap_or_default();
            filter.filter(&file_tags)
        })
        .collect();

    if filenames.is_empty() && !hook.always_run {
        writeln!(
            printer.stdout(),
            "{}",
            line(&hook.name, cols, SKIPPED, Style::new().yellow(), NO_FILES)
        )?;
        return Ok((true, diff));
    }

    write!(
        printer.stdout(),
        "{}{}",
        &hook.name,
        ".".repeat(cols - hook.name.len() - 6 - 1)
    )?;
    let start = std::time::Instant::now();

    let output = if hook.pass_filenames {
        hook.language.run(hook, &filenames).await?
    } else {
        hook.language.run(hook, &[]).await?
    };

    let duration = start.elapsed();

    let new_diff = get_diff().await?;
    let file_modified = diff != new_diff;
    let success = output.status.success() && !file_modified;

    if success {
        writeln!(printer.stdout(), "{}", "Passed".green())?;
    } else {
        writeln!(printer.stdout(), "{}", "Failed".red())?;
    }

    if verbose || hook.verbose || !success {
        writeln!(
            printer.stdout(),
            "{}",
            format!("- hook id: {}", hook.id).dimmed()
        )?;
        if verbose || hook.verbose {
            writeln!(
                printer.stdout(),
                "{}",
                format!("- duration: {:?}s", duration.as_secs()).dimmed()
            )?;
        }
        if !output.status.success() {
            writeln!(
                printer.stdout(),
                "{}",
                format!("- exit code: {}", output.status.code().unwrap_or_default()).dimmed()
            )?;
        }
        if file_modified {
            writeln!(
                printer.stdout(),
                "{}",
                "- files were modified by this hook".dimmed()
            )?;
        }
        if !(output.stdout.is_empty() && output.stderr.is_empty()) {
            // TODO: write to log file
            std::io::stdout().write_all(&output.stdout)?;
            std::io::stderr().write_all(&output.stderr)?;
        }
        // TODO: output
    }

    Ok((success, new_diff))
}

fn calculate_columns(hooks: &[Hook]) -> usize {
    let name_len = hooks.iter().map(|hook| hook.id.len()).max().unwrap_or(0);
    max(80, name_len + 3 + NO_FILES.len() + 1 + SKIPPED.len())
}

pub async fn run_hooks(
    hooks: &[Hook],
    skips: &[String],
    filenames: Vec<&String>,
    fail_fast: bool,
    show_diff_on_failure: bool,
    verbose: bool,
    printer: Printer,
) -> Result<()> {
    let columns = calculate_columns(hooks);
    // TODO: progress bar, format output
    let mut success = true;

    let mut diff = get_diff().await?;
    // hooks must run in serial
    for hook in hooks {
        // TODO: handle single hook result
        let (hook_success, new_diff) =
            run_hook(hook, &filenames, skips, diff, columns, verbose, printer).await?;

        success |= hook_success;
        diff = new_diff;
        if !success && (fail_fast || hook.fail_fast) {
            break;
        }
    }

    if !success && show_diff_on_failure {
        writeln!(printer.stdout(), "All changes made by hooks:")?;
        // TODO: color
        Command::new("git")
            .arg("diff")
            .arg("--no-ext-diff")
            .arg("--no-pager")
            .spawn()?
            .wait()
            .await?;
    };

    Ok(())
}

pub fn filter_filenames<'a>(
    filenames: impl ParallelIterator<Item = &'a String>,
    include: Option<&str>,
    exclude: Option<&str>,
) -> Result<impl ParallelIterator<Item = &'a String>, Error> {
    // TODO: normalize path separators
    let include = include.map(|s| Regex::new(s)).transpose()?;
    let exclude = exclude.map(|s| Regex::new(s)).transpose()?;

    Ok(filenames.filter(move |filename| {
        let filename = filename.as_str();
        if !include
            .as_ref()
            .map(|re| re.is_match(filename))
            .unwrap_or(true)
        {
            return false;
        }
        if exclude
            .as_ref()
            .map(|re| re.is_match(filename))
            .unwrap_or(false)
        {
            return false;
        }
        true
    }))
}
