use std::process::Output;

use crate::hook::Hook;
use crate::languages::DEFAULT_VERSION;

pub struct System;

impl System {
    pub fn name(&self) -> &str {
        "System"
    }

    pub fn default_version(&self) -> &str {
        DEFAULT_VERSION
    }

    pub fn environment_dir(&self) -> Option<&str> {
        None
    }

    pub async fn install(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }

    pub async fn run(&self, _hook: &Hook, _filenames: &[&String]) -> anyhow::Result<Output> {
        todo!()
    }
}
