use std::path::PathBuf;

use anyhow::Result;

use crate::cli::ExitStatus;
use crate::config::Stage;
use crate::hook::Project;
use crate::store::Store;

pub(crate) async fn run(
    config: Option<PathBuf>,
    hook: Option<String>,
    hook_stage: Option<Stage>,
) -> Result<ExitStatus> {
    let store = Store::from_settings()?.init()?;
    let project = Project::current(config)?;

    let hooks = project.hooks(&store).await?;

    let hooks: Vec<_> = hooks
        .into_iter()
        .filter(|h| {
            if let Some(ref hook) = hook {
                &h.id == hook || h.alias.as_ref() == Some(hook)
            } else {
                true
            }
        })
        .filter(|h| match (hook_stage, h.stages.as_ref()) {
            (Some(ref stage), Some(stages)) => stages.contains(stage),
            (_, _) => true,
        })
        .collect();

    if hooks.is_empty() && hook.is_some() {
        if let Some(hook_stage) = hook_stage {
            eprintln!(
                "No hooks found for hook ID `{}` and stage `{:?}`",
                hook.unwrap(),
                hook_stage
            );
        } else {
            eprintln!("No hooks found for hook ID: {}", hook.unwrap());
        }
        return Ok(ExitStatus::Failure);
    }

    for hook in hooks {
        println!("Running hook: {}", hook.id);
    }

    Ok(ExitStatus::Success)
}
