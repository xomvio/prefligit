use anyhow::Result;
use std::path::PathBuf;

use crate::cli::ExitStatus;
use crate::config::{read_config, CONFIG_FILE};
use crate::store::Store;

pub(crate) fn run(config: Option<PathBuf>) -> Result<ExitStatus> {
    let _store = Store::from_settings(None)?;
    let _config = read_config(&config.unwrap_or_else(|| PathBuf::from(CONFIG_FILE)))?;

    // let hooks = config.repos

    Ok(ExitStatus::Success)
}
