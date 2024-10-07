use clap::{arg, ArgAction, Args, ColorChoice, Parser, Subcommand, ValueEnum};
use std::{path::PathBuf, process::ExitCode};
use crate::config::Stage;

mod config;

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Compat(CompatNamespace),
}

#[derive(Args, Debug)]
struct CompatNamespace {
    #[command(flatten)]
    global_args: CompatGlobalArgs,

    #[command(subcommand)]
    command: CompatCommand,
}

#[derive(Subcommand, Debug)]
enum CompatCommand {
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
struct CompatGlobalArgs {
    /// Whether to use color in output.
    #[arg(global = true, long, value_enum, default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    /// Path to alternate config file.
    #[arg(global = true, short, long, value_parser)]
    config: Option<PathBuf>,

    /// Use verbose output.
    #[arg(global = true, short, long, action = ArgAction::Count)]
    verbose: u8,
}

#[derive(Args, Debug)]
struct InstallArgs {
    /// Overwrite existing hooks.
    #[arg(short = 'f', long)]
    overwrite: bool,

    /// Install hook environments.
    #[arg(long)]
    install_hooks: bool,

    #[arg(short = 't', long, value_enum)]
    hook_type: Vec<HookType>,

    /// Whether to allow a missing `pre-commit` configuration file or exit with a failure code.
    #[arg(long)]
    allow_missing_config: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum HookType {
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
struct RunArgs {
    #[arg(required = true)]
    pub hook_id: Vec<String>,
    #[arg(short, long)]
    pub all_files: bool,
    #[arg(long, conflicts_with = "all_files")]
    pub files: Vec<PathBuf>,
    #[arg(long, requires = "to_ref")]
    pub from_ref: Option<String>,
    #[arg(long, requires = "from_ref")]
    pub to_ref: Option<String>,
    #[arg(long)]
    pub hook_stage: Option<Stage>,
    #[arg(long)]
    pub show_diff_on_failure: bool,
}

#[derive(Args, Debug)]
struct AutoUpdateArgs {
    #[arg(long, default_value_t = true)]
    pub bleeding_edge: bool,
    #[arg(long)]
    pub freeze: bool,
    #[arg(long)]
    pub repo: Option<String>,
    #[arg(short, long, default_value_t = 1)]
    pub jobs: usize,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            eprintln!("{}", err);
            return ExitCode::FAILURE;
        }
    };

    match cli.command {
        Commands::Compat(command) => match command.command {
            CompatCommand::Install(options) => {
                println!("Installing with options: {:?}", options);
            }
            CompatCommand::InstallHooks => {
                println!("Installing hooks");
            }
            CompatCommand::Run(options) => {
                println!("Running with options: {:?}", options);
            }
            _ => {
                eprintln!("Command not implemented yet");
                return ExitCode::FAILURE;
            }
        },
    };

    ExitCode::SUCCESS
}
