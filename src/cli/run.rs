use std::fmt::Write;
use std::path::PathBuf;

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::cli::ExitStatus;
use crate::config::Stage;
use crate::hook::{install_hooks, run_hooks, Hook, Project};
use crate::printer::Printer;
use crate::store::Store;

pub(crate) async fn run(
    config: Option<PathBuf>,
    hook_id: Option<String>,
    hook_stage: Option<Stage>,
    printer: Printer,
) -> Result<ExitStatus> {
    // TODO: find git root

    let store = Store::from_settings()?.init()?;
    let project = Project::current(config)?;

    // TODO: check .pre-commit-config.yaml status and git status
    // TODO: fill env vars
    // TODO: impl staged_files_only

    let lock = store.lock_async().await?;
    let hooks = project.prepare_hooks(&store, printer).await?;

    let hooks: Vec<_> = hooks
        .into_iter()
        .filter(|h| {
            if let Some(ref hook) = hook_id {
                &h.id == hook || h.alias.as_ref() == Some(hook)
            } else {
                true
            }
        })
        .filter(|h| {
            if let Some(stage) = hook_stage {
                h.stages.contains(&stage)
            } else {
                true
            }
        })
        .collect();

    if hooks.is_empty() && hook_id.is_some() {
        if let Some(hook_stage) = hook_stage {
            writeln!(
                printer.stderr(),
                "No hook found for id `{}` and stage `{}`",
                hook_id.unwrap().cyan(),
                hook_stage.cyan()
            )?;
        } else {
            writeln!(
                printer.stderr(),
                "No hook found for id `{}`",
                hook_id.unwrap().cyan()
            )?;
        }
        return Ok(ExitStatus::Failure);
    }

    let hooks = apply_skips(hooks);

    install_hooks(&hooks, printer).await?;
    drop(lock);

    run_hooks(&hooks, printer).await?;

    for hook in hooks {
        writeln!(
            printer.stdout(),
            "Running hook `{}` at `{}`",
            hook.to_string().cyan(),
            hook.path().to_string_lossy().dimmed()
        )?;
    }

    Ok(ExitStatus::Success)
}

fn apply_skips(hooks: Vec<Hook>) -> Vec<Hook> {
    let skips = match std::env::var_os("SKIP") {
        Some(s) if !s.is_empty() => s
            .to_string_lossy()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>(),
        _ => return hooks,
    };

    hooks
        .into_iter()
        .filter(|h| !skips.contains(&h.id))
        .collect()
}
