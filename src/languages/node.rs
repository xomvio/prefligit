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
        Some("node-env")
    }

    pub async fn install(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }

    pub async fn run(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }
}
