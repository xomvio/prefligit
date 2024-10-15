use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use anyhow::Result;
use assert_cmd::output::{OutputError, OutputOkExt};
use tokio::process::Command;
use tracing::warn;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Command(#[from] OutputError),
    #[error("Failed to find git: {0}")]
    GitNotFound(#[from] which::Error),
}

static GIT: LazyLock<Result<PathBuf, which::Error>> = LazyLock::new(|| which::which("git"));

static GIT_ENV: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let keep = &[
        "GIT_EXEC_PATH",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_SSL_CAINFO",
        "GIT_SSL_NO_VERIFY",
        "GIT_CONFIG_COUNT",
        "GIT_HTTP_PROXY_AUTHMETHOD",
        "GIT_ALLOW_PROTOCOL",
        "GIT_ASKPASS",
    ];

    std::env::vars()
        .filter(|(k, _)| {
            !k.starts_with("GIT_")
                || k.starts_with("GIT_CONFIG_KEY_")
                || k.starts_with("GIT_CONFIG_VALUE_")
                || keep.contains(&k.as_str())
        })
        .collect()
});

fn git_cmd() -> Result<Command, Error> {
    let mut cmd = Command::new(GIT.clone()?);
    cmd.arg("-c").arg("core.useBuiltinFSMonitor=false");
    cmd.envs(GIT_ENV.iter().cloned());

    Ok(cmd)
}

pub async fn has_unmerged_paths(path: &Path) -> Result<bool, Error> {
    let output = git_cmd()?
        .arg("ls-files")
        .arg("--unmerged")
        .current_dir(path)
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub async fn is_dirty(path: &Path) -> Result<bool, Error> {
    let output = git_cmd()?
        .arg("diff")
        .arg("--quiet") // Implies `--exit-code`
        .arg("--no-ext-diff") // Disable external diff drivers
        .arg(path)
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok();
    match output {
        Ok(_) => Ok(false),
        Err(err) => {
            if err
                .as_output()
                .is_some_and(|output| output.status.code() == Some(1))
            {
                Ok(true)
            } else {
                Err(err.into())
            }
        }
    }
}

async fn init_repo(url: &str, path: &Path) -> Result<(), Error> {
    git_cmd()?
        .arg("init")
        .arg("--template=")
        .arg(path)
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(url)
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    Ok(())
}

async fn shallow_clone(rev: &str, path: &Path) -> Result<(), Error> {
    git_cmd()?
        .current_dir(path)
        .arg("-c")
        .arg("protocol.version=2")
        .arg("fetch")
        .arg("origin")
        .arg(rev)
        .arg("--depth=1")
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("checkout")
        .arg("FETCH_HEAD")
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("-c")
        .arg("protocol.version=2")
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--recursive")
        .arg("--depth=1")
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    Ok(())
}

async fn full_clone(rev: &str, path: &Path) -> Result<(), Error> {
    git_cmd()?
        .current_dir(path)
        .arg("fetch")
        .arg("origin")
        .arg("--tags")
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("checkout")
        .arg(rev)
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--recursive")
        .output()
        .await
        .map_err(OutputError::with_cause)?
        .ok()?;

    Ok(())
}

pub async fn clone_repo(url: &str, rev: &str, path: &Path) -> Result<(), Error> {
    init_repo(url, path).await?;

    if let Err(err) = shallow_clone(rev, path).await {
        warn!(?err, "Failed to shallow clone, falling back to full clone");
        full_clone(rev, path).await
    } else {
        Ok(())
    }
}
