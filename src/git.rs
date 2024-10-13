use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;
use assert_cmd::output::OutputOkExt;
use tracing::warn;

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

fn git_cmd() -> Result<Command> {
    let mut cmd = Command::new(GIT.deref().as_ref()?);
    cmd.arg("-c").arg("core.useBuiltinFSMonitor=false");
    cmd.envs(GIT_ENV.iter().cloned());

    Ok(cmd)
}

pub fn has_unmerged_paths(path: &Path) -> Result<bool> {
    let output = git_cmd()?
        .arg("ls-files")
        .arg("--unmerged")
        .current_dir(path)
        .ok()?;
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub fn is_dirty(path: &Path) -> Result<bool> {
    let output = git_cmd()?
        .arg("diff")
        .arg("--quiet") // Implies `--exit-code`
        .arg("--no-ext-diff") // Disable external diff drivers
        .arg(path)
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

pub fn init_repo(url: &str, path: &Path) -> Result<()> {
    git_cmd()?.arg("init").arg("--template=").arg(path).ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(url)
        .ok()?;

    Ok(())
}

fn shallow_clone(rev: &str, path: &Path) -> Result<()> {
    git_cmd()?
        .current_dir(path)
        .arg("-c")
        .arg("protocol.version=2")
        .arg("fetch")
        .arg("origin")
        .arg(rev)
        .arg("--depth=1")
        .ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("checkout")
        .arg("FETCH_HEAD")
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
        .ok()?;

    Ok(())
}

fn full_clone(rev: &str, path: &Path) -> Result<()> {
    git_cmd()?
        .current_dir(path)
        .arg("fetch")
        .arg("origin")
        .arg("--tags")
        .ok()?;

    git_cmd()?.current_dir(path).arg("checkout").arg(rev).ok()?;

    git_cmd()?
        .current_dir(path)
        .arg("submodule")
        .arg("update")
        .arg("--init")
        .arg("--recursive")
        .ok()?;

    Ok(())
}

pub fn clone_repo(url: &str, rev: &str, path: &Path) -> Result<()> {
    init_repo(url, path)?;

    shallow_clone(rev, path)
        .inspect_err(|err| {
            warn!(?err, "Failed to shallow clone, falling back to full clone");
        })
        .or_else(|_| full_clone(rev, path))
}
