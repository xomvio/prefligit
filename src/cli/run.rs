use anyhow::Result;

use crate::cli::ExitStatus;

pub(crate) fn run() -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
