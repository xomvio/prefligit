use std::path::PathBuf;

use anyhow::Result;

use crate::config::{read_config, ConfigWire, HookWire, RepoLocation, RepoWire, CONFIG_FILE};
use crate::fs::CWD;
use crate::store::{Repo, Store};

pub struct Project {
    root: PathBuf,
    config: ConfigWire,
}

impl Project {
    pub fn from_directory(root: PathBuf, config: Option<PathBuf>) -> Result<Self> {
        let config_path = config.unwrap_or_else(|| root.join(CONFIG_FILE));
        let config = read_config(&config_path)?;
        Ok(Self { root, config })
    }

    pub fn current(config: Option<PathBuf>) -> Result<Self> {
        Self::from_directory(CWD.clone(), config)
    }

    pub fn repos(&self, store: &Store) -> Result<Vec<&Repo>> {
        // TODO: 并行初始化 repo
        self.config
            .repos
            .iter()
            .map(|repo| store.init_repo(repo))
            .collect()
    }

    pub fn hooks(&self, store: &Store) -> Result<Vec<&HookWire>> {
        self.config.repos.iter().flat_map(|repo| {
            let store_repo = store.init_repo(repo)?;

            // Check hook id is valid
            repo.hooks.iter().for_each(|hook| {
                store_repo.hooks()?.get(&hook.id).ok_or_else(|| {
                    anyhow::anyhow!("Hook `{}` not found in repo `{}`", hook.id, repo)
                })
            });

            Ok(repo.hooks.iter())
        }).collect()
    }
}
