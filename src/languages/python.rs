use assert_cmd::output::{OutputError, OutputOkExt};
use tokio::process::Command;

use crate::hook::Hook;

pub struct Python;

impl Python {
    pub fn name(&self) -> &str {
        "Python"
    }

    pub fn default_version(&self) -> &str {
        // TODO find the version of python on the system
        "python3"
    }

    pub fn environment_dir(&self) -> Option<&str> {
        Some("py_env")
    }

    // TODO: install uv automatically
    // TODO: fallback to pip
    pub async fn install(&self, hook: &Hook) -> anyhow::Result<()> {
        let venv = hook.environment_dir().expect("No environment dir found");
        // Create venv
        Command::new("uv")
            .arg("venv")
            .arg(&venv)
            .arg("--python")
            .arg(&hook.language_version)
            .output()
            .await
            .map_err(OutputError::with_cause)?
            .ok()?;

        // Install dependencies
        Command::new("uv")
            .arg("pip")
            .arg("install")
            .arg(".")
            .args(&hook.additional_dependencies)
            .current_dir(hook.path())
            .env("VIRTUAL_ENV", &venv)
            .output()
            .await
            .map_err(OutputError::with_cause)?
            .ok()?;

        Ok(())
    }

    pub async fn run(&self, _hook: &Hook) -> anyhow::Result<()> {
        todo!()
    }
}
