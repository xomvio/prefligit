use std::fmt::Write;
use std::path::PathBuf;

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::cli::ExitStatus;
use crate::config::Stage;
use crate::hook::Project;
use crate::printer::Printer;
use crate::store::Store;

pub(crate) async fn run(
    config: Option<PathBuf>,
    hook_id: Option<String>,
    hook_stage: Option<Stage>,
    printer: Printer,
) -> Result<ExitStatus> {
    let store = Store::from_settings()?.init()?;
    let project = Project::current(config)?;

    let lock = store.lock_async().await?;
    let hooks = project.hooks(&store).await?;

    let hooks: Vec<_> = hooks
        .into_iter()
        .filter(|h| {
            if let Some(ref hook) = hook_id {
                &h.id() == hook || h.alias() == Some(hook)
            } else {
                true
            }
        })
        .filter(|h| match (hook_stage, h.stages()) {
            (Some(ref stage), Some(stages)) => stages.contains(stage),
            (_, _) => true,
        })
        .collect();

    if hooks.is_empty() && hook_id.is_some() {
        if let Some(hook_stage) = hook_stage {
            writeln!(
                printer.stderr(),
                "No hook found for id `{}` and stage `{:?}`",
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

    // TODO: apply skips

    // store.install_hooks(&hooks).await?;
    drop(lock);

    for hook in hooks {
        writeln!(
            printer.stdout(),
            "Running hook `{}` at `{}`",
            hook.id().cyan(),
            hook.path().to_string_lossy().dimmed()
        )?;
    }

    Ok(ExitStatus::Success)
}
