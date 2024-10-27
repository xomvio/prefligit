use std::path::{Path, PathBuf};

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

    pub async fn run(&self, hook: &Hook, filenames: &[&String]) -> anyhow::Result<()> {
        // Construct the `PATH` environment variable.
        let env = hook.environment_dir().unwrap();

        let new_path = std::env::join_paths(
            std::iter::once(bin_dir(env.as_path())).chain(
                std::env::var_os("PATH")
                    .as_ref()
                    .iter()
                    .flat_map(std::env::split_paths),
            ),
        )?;

        // TODO: handle signals
        // TODO: better error display
        let cmds = shlex::split(&hook.entry).ok_or(anyhow::anyhow!("Failed to parse entry"))?;
        Command::new(&cmds[0])
            .args(&cmds[1..])
            .args(&hook.args)
            .args(filenames)
            .env("VIRTUAL_ENV", &env)
            .env("PATH", new_path)
            .env_remove("PYTHONHOME")
            .output()
            .await
            .map_err(OutputError::with_cause)?
            .ok()?;

        Ok(())
    }
}

fn bin_dir(venv: &Path) -> PathBuf {
    if cfg!(windows) {
        venv.join("Scripts")
    } else {
        venv.join("bin")
    }
}
