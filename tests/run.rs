use anyhow::Result;
use assert_fs::prelude::*;

use crate::common::{cmd_snapshot, TestContext};

mod common;

#[test]
fn run() -> Result<()> {
    let context = TestContext::new();

    fs_err::copy(
        "tests/files/uv-pre-commit-config.yaml",
        context.workdir().child(".pre-commit-config.yaml"),
    )?;

    cmd_snapshot!(context.filters(), context.run(), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Running hook: validate-pyproject
    Running hook: typos
    Running hook: cargo-fmt
    Running hook: cargo-dev-generate-all
    Running hook: prettier
    Running hook: ruff-format
    Running hook: ruff

    ----- stderr -----
    "###);

    cmd_snapshot!(context.filters(), context.run().arg("typos"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Running hook: typos

    ----- stderr -----
    "###);

    cmd_snapshot!(context.filters(), context.run().arg("typos").arg("--hook-stage").arg("pre-push"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Running hook: typos

    ----- stderr -----
    "###);

    Ok(())
}

#[test]
fn invalid_hook_id() -> Result<()> {
    let context = TestContext::new();

    fs_err::write(
        context.workdir().child(".pre-commit-config.yaml"),
        indoc::indoc! {r#"
            repos:
              - repo: https://github.com/abravalheri/validate-pyproject
                rev: v0.20.2
                hooks:
                  - id: invalid-hook-id
            "#
        },
    )?;

    cmd_snapshot!(context.filters(), context.run().arg("invalid-hook-id"), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: Hook not found: invalid-hook-id in repo https://github.com/abravalheri/validate-pyproject@v0.20.2
    "###);

    Ok(())
}
