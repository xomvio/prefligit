use std::path::PathBuf;

use anyhow::Result;

use crate::cli::ExitStatus;
use crate::config::Stage;
use crate::hook::Project;
use crate::store::Store;

pub(crate) async fn run(
    config: Option<PathBuf>,
    hook_id: Option<String>,
    hook_stage: Option<Stage>,
) -> Result<ExitStatus> {
    let store = Store::from_settings()?.init()?;
    let project = Project::current(config)?;

    let lock = store.lock_async().await?;
    let hooks = project.hooks(&store).await?;

    let hooks: Vec<_> = hooks
        .into_iter()
        .filter(|h| {
            if let Some(ref hook) = hook_id {
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

    if hooks.is_empty() && hook_id.is_some() {
        if let Some(hook_stage) = hook_stage {
            eprintln!(
                "No hook found for id `{}` and stage `{:?}`",
                hook_id.unwrap(),
                hook_stage
            );
        } else {
            eprintln!("No hook found for id {}", hook_id.unwrap());
        }
        return Ok(ExitStatus::Failure);
    }

    // store.install_hooks(&hooks).await?;
    drop(lock);

    for hook in hooks {
        println!(
            "Running hook: {} at {}",
            hook.id,
            hook.path().to_string_lossy()
        );
    }

    Ok(ExitStatus::Success)
}
