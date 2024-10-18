use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{ExitStatus, HookType};
use crate::printer::Printer;

pub(crate) async fn install(
    _config: Option<PathBuf>,
    _hook_type: Vec<HookType>,
    _install_hooks: bool,
    _printer: Printer,
) -> Result<ExitStatus> {
    todo!()
}
