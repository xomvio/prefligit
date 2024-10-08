use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;

static GIT: LazyLock<PathBuf> =
    LazyLock::new(|| which::which("git").expect("`git` not found in PATH"));

static GIT_ENV: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let keep =  &[
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

fn git_cmd() -> Command {
    let mut cmd = Command::new(&*GIT);
    cmd.arg("-c").arg("core.useBuiltinFSMonitor=false");
    cmd.envs(GIT_ENV.iter().cloned());

    cmd
}

pub fn has_unmerged_paths(path: &Path) -> Result<bool> {
    let output = git_cmd()
        .arg("ls-files")
        .arg("--unmerged")
        .current_dir(path)
        .output()?;
    if !output.status.success() {
        anyhow::bail!(
            "`git ls-files` failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

pub fn is_dirty(path: &Path) -> Result<bool> {
    let output = git_cmd()
        .arg("diff")
        .arg("--quiet") // Implies `--exit-code`
        .arg("--no-ext-diff") // Disable external diff drivers
        .arg(path)
        .output()?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => anyhow::bail!(
            "`git diff` failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ),
    }
}
