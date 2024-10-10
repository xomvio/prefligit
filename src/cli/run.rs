use std::path::PathBuf;

use anyhow::Result;

use crate::cli::ExitStatus;
use crate::config::{read_config, ConfigWire, RepoWire, Stage, CONFIG_FILE};
use crate::fs::CWD;
use crate::store::Store;

pub struct Repository {
    root: PathBuf,
    config: ConfigWire,
}

impl Repository {
    pub fn from_directory(root: PathBuf, config: Option<PathBuf>) -> Result<Self> {
        let config_path = config.unwrap_or_else(|| root.join(CONFIG_FILE));
        let config = read_config(&config_path)?;
        Ok(Self { root, config })
    }

    pub fn current(config: Option<PathBuf>) -> Result<Self> {
        Self::from_directory(CWD.clone(), config)
    }

    pub fn repos(&self) -> Vec<&RepoWire> {
        self.config.repos.iter().collect()
    }
}

pub(crate) fn run(
    config: Option<PathBuf>,
    hook: Option<String>,
    hook_stage: Option<Stage>,
) -> Result<ExitStatus> {
    let _store = Store::from_settings(None)?;
    let repo = Repository::current(config)?;

    let hooks: Vec<_> = repo
        .repos()
        .iter()
        .flat_map(|repo| repo.hooks.iter())
        .filter(|&h| {
            if let Some(ref hook) = hook {
                &h.id == hook || h.alias.as_ref() == Some(hook)
            } else {
                true
            }
        })
        .filter(|&h| match (hook_stage, h.stages.as_ref()) {
            (Some(ref stage), Some(stages)) => stages.contains(stage),
            (_, _) => true,
        })
        .collect();

    for hook in hooks {
        println!("Running hook: {}", hook.id);
    }

    Ok(ExitStatus::Success)
}
