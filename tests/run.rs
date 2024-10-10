use anyhow::Result;
use assert_fs::prelude::*;

use crate::common::{cmd_snapshot, TestContext};

mod common;

#[test]
fn run() -> Result<()> {
    let context = TestContext::new();
    context
        .temp_dir
        .child(".pre-commit-config.yaml")
        .write_str("repos: []")?;

    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----

    ----- stderr -----
    "#);
    Ok(())
}
