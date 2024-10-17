use crate::hook::Hook;
use crate::languages::DEFAULT_VERSION;

pub struct Python;

impl Python {
    pub fn name(&self) -> &str {
        "Python"
    }

    pub fn default_version(&self) -> &str {
        DEFAULT_VERSION
    }

    pub fn environment_dir(&self) -> Option<&str> {
        Some("py-env")
    }

    pub async fn install(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }

    pub async fn run(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }
}
