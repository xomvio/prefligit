use std::{path::PathBuf, process::ExitCode};
use clap::{arg, ArgAction, Args, ColorChoice, Parser, Subcommand, ValueEnum};

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
    InstallHooks(InstallHooksArgs),
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
    AutoUpdate,
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
    // Add fields for run options here
}

#[derive(Args, Debug)]
struct InstallHooksArgs {

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
        Commands::Compat(command) => {
            match command.command {
                CompatCommand::Install(options) => {
                    println!("Installing with options: {:?}", options);
                }
                CompatCommand::InstallHooks(options) => {
                    println!("Installing hooks with options: {:?}", options);
                }
                CompatCommand::Run(options) => {
                    println!("Running with options: {:?}", options);
                }
                _ => {
                    eprintln!("Command not implemented yet");
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    ExitCode::SUCCESS
}
