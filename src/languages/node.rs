use std::process::{ExitStatus, Output};

use crate::hook::Hook;
use crate::languages::DEFAULT_VERSION;

pub struct Node;

impl Node {
    pub fn name(&self) -> &str {
        "Node"
    }

    pub fn default_version(&self) -> &str {
        DEFAULT_VERSION
    }

    pub fn environment_dir(&self) -> Option<&str> {
        Some("node_env")
    }

    pub async fn install(&self, hook: &Hook) -> anyhow::Result<()> {
        // TODO: install node automatically
        let env = hook.environment_dir().expect("No environment dir found");
        fs_err::create_dir_all(env)?;
        Ok(())
    }

    pub async fn run(&self, _hook: &Hook, _filenames: &[&String]) -> anyhow::Result<Output> {
        Ok(Output {
            status: ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}
