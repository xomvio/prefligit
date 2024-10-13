use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{ExitStatus, HookType};

pub(crate) fn install(
    _config: Option<PathBuf>,
    _hook_type: Vec<HookType>,
    _install_hooks: bool,
) -> Result<ExitStatus> {
    todo!()
}
