use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Args, ColorChoice, Parser, Subcommand, ValueEnum};

use crate::config::Stage;

mod run;

pub(crate) use run::run;

#[derive(Copy, Clone)]
pub(crate) enum ExitStatus {
    /// The command succeeded.
    Success,

    /// The command failed due to an error in the user input.
    Failure,

    /// The command failed with an unexpected error.
    Error,

    Interrupted,

    /// The command's exit status is propagated from an external command.
    External(u8),
}

impl From<ExitStatus> for ExitCode {
    fn from(status: ExitStatus) -> Self {
        match status {
            ExitStatus::Success => Self::from(0),
            ExitStatus::Failure => Self::from(1),
            ExitStatus::Error => Self::from(2),
            ExitStatus::Interrupted => Self::from(130),
            ExitStatus::External(code) => Self::from(code),
        }
    }
}

#[derive(Parser, Debug)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    Compat(CompatNamespace),
}

#[derive(Args, Debug)]
pub(crate) struct CompatNamespace {
    #[command(flatten)]
    pub(crate) global_args: CompatGlobalArgs,

    #[command(subcommand)]
    pub(crate) command: CompatCommand,
}

#[derive(Subcommand, Debug)]
pub(crate) enum CompatCommand {
    /// Install the git pre-commit hooks.
    Install(InstallArgs),
    /// Install hook environments for all hooks used in the config file.
    InstallHooks,
    /// Run hooks.
    Run(RunArgs),
    /// Uninstall the pre-commit script.
    Uninstall,
    /// Validate `.pre-commit-config.yaml` files.
    ValidateConfig,
    /// Validate `.pre-commit-hooks.yaml` files.
    ValidateManifest,
    /// Produce a sample `.pre-commit-config.yaml` file.
    SampleConfig,
    /// Auto-update pre-commit config to the latest repos' versions.
    #[command(name = "autoupdate")]
    AutoUpdate(AutoUpdateArgs),
    /// Clean unused cached repos.
    GC,
    /// Clean out pre-commit files.
    Clean,
    /// Install hook script in a directory intended for use with `git config init.templateDir`.
    #[command(name = "init-templatedir")]
    InitTemplateDir,
    /// Try the pre-commit hooks in the current repo.
    TryRepo,
}

#[derive(Parser, Debug)]
pub(crate) struct CompatGlobalArgs {
    /// Whether to use color in output.
    #[arg(global = true, long, value_enum, default_value_t = ColorChoice::Auto)]
    pub(crate) color: ColorChoice,

    /// Path to alternate config file.
    #[arg(global = true, short, long, value_parser)]
    pub(crate) config: Option<PathBuf>,

    /// Use verbose output.
    #[arg(global = true, short, long, action = ArgAction::Count)]
    pub(crate) verbose: u8,
}

#[derive(Args, Debug)]
pub(crate) struct InstallArgs {
    /// Overwrite existing hooks.
    #[arg(short = 'f', long)]
    pub(crate) overwrite: bool,

    /// Install hook environments.
    #[arg(long)]
    pub(crate) install_hooks: bool,

    #[arg(short = 't', long, value_enum)]
    pub(crate) hook_type: Vec<HookType>,

    /// Whether to allow a missing `pre-commit` configuration file or exit with a failure code.
    #[arg(long)]
    pub(crate) allow_missing_config: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum HookType {
    CommitMsg,
    PostCheckout,
    PostCommit,
    PostMerge,
    PostRewrite,
    PreCommit,
    PreMergeCommit,
    PrePush,
    PreRebase,
    PrepareCommitMsg,
}

#[derive(Args, Debug)]
pub(crate) struct RunArgs {
    /// The hook ID to run.
    #[arg(value_name = "HOOK")]
    pub(crate) hook_id: Option<String>,
    #[arg(short, long)]
    pub(crate) all_files: bool,
    #[arg(long, conflicts_with = "all_files")]
    pub(crate) files: Vec<PathBuf>,
    #[arg(long, requires = "to_ref")]
    pub(crate) from_ref: Option<String>,
    #[arg(long, requires = "from_ref")]
    pub(crate) to_ref: Option<String>,
    #[arg(long)]
    pub(crate) hook_stage: Option<Stage>,
    #[arg(long)]
    pub(crate) show_diff_on_failure: bool,
}

#[derive(Args, Debug)]
pub(crate) struct AutoUpdateArgs {
    #[arg(long, default_value_t = true)]
    pub(crate) bleeding_edge: bool,
    #[arg(long)]
    pub(crate) freeze: bool,
    #[arg(long)]
    pub(crate) repo: Option<String>,
    #[arg(short, long, default_value_t = 1)]
    pub(crate) jobs: usize,
}
