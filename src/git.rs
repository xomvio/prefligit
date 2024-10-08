use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Result;

static GIT: LazyLock<PathBuf> =
    LazyLock::new(|| which::which("git").expect("`git` not found in PATH"));

pub fn has_unmerged_paths(path: &Path) -> Result<bool> {
    let output = Command::new(&*GIT)
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
    let output = Command::new(&*GIT)
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
