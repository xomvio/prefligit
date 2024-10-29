use std::process::Output;

use crate::config;
use crate::hook::Hook;
use crate::languages::{LanguageImpl, DEFAULT_VERSION};

#[derive(Debug, Copy, Clone)]
pub struct System;

impl LanguageImpl for System {
    fn name(&self) -> config::Language {
        config::Language::System
    }

    fn default_version(&self) -> &str {
        DEFAULT_VERSION
    }

    fn environment_dir(&self) -> Option<&str> {
        None
    }

    async fn install(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }

    async fn check_health(&self) -> anyhow::Result<()> {
        todo!()
    }

    async fn run(&self, _hook: &Hook, _filenames: &[&String]) -> anyhow::Result<Output> {
        Ok(Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}
