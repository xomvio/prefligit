use anyhow::Result;
use assert_cmd::Command;
use assert_fs::prelude::*;

use crate::common::{cmd_snapshot, TestContext};

mod common;

#[test]
fn run_basic() -> Result<()> {
    let context = TestContext::new();

    let cwd = context.workdir();
    cwd.child(".pre-commit-config.yaml")
        .write_str(indoc::indoc! {r#"
            repos:
              - repo: https://github.com/pre-commit/pre-commit-hooks
                rev: v5.0.0
                hooks:
                  - id: trailing-whitespace
                  - id: end-of-file-fixer
                  - id: check-json
        "#})?;

    // Create a repository with some files.
    cwd.child("file.txt").write_str("Hello, world!\n")?;
    cwd.child("valid.json").write_str("{}")?;
    cwd.child("invalid.json").write_str("{}")?;
    cwd.child("main.py").write_str(r#"print "abc"  "#)?;
    Command::new("git")
        .current_dir(cwd)
        .arg("init")
        .assert()
        .success();
    Command::new("git")
        .current_dir(cwd)
        .arg("add")
        .arg(".")
        .assert()
        .success();

    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    Cloning https://github.com/pre-commit/pre-commit-hooks@v5.0.0
    Installing environment for https://github.com/pre-commit/pre-commit-hooks@v5.0.0
    trim trailing whitespace.................................................Failed
    - hook id: trailing-whitespace
    - exit code: 1
    - files were modified by this hook
    Fixing main.py
    fix end of files.........................................................Failed
    - hook id: end-of-file-fixer
    - exit code: 1
    - files were modified by this hook
    Fixing invalid.json
    Fixing main.py
    Fixing valid.json
    check json...............................................................Passed

    ----- stderr -----
    "#);

    cmd_snapshot!(context.filters(), context.run().arg("trailing-whitespace"), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    trim trailing whitespace.................................................Passed

    ----- stderr -----
    "#);

    cmd_snapshot!(context.filters(), context.run().arg("typos").arg("--hook-stage").arg("pre-push"), @r#"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    No hook found for id `typos` and stage `pre-push`
    "#);

    Ok(())
}

#[test]
fn invalid_hook_id() -> Result<()> {
    let context = TestContext::new();

    context.init_project()?;

    context
        .workdir()
        .child(".pre-commit-config.yaml")
        .write_str(indoc::indoc! {r#"
            repos:
              - repo: https://github.com/pre-commit/pre-commit-hooks
                rev: v5.0.0
                hooks:
                  - id: trailing-whitespace
            "#
        })?;

    cmd_snapshot!(context.filters(), context.run().arg("invalid-hook-id"), @r#"
    success: false
    exit_code: 1
    ----- stdout -----

    ----- stderr -----
    No hook found for id `invalid-hook-id`
    "#);

    Ok(())
}

/// Test the output format for a hook with a CJK name.
#[test]
fn cjk_hook_name() -> Result<()> {
    let context = TestContext::new();

    context.init_project()?;

    context
        .workdir()
        .child(".pre-commit-config.yaml")
        .write_str(indoc::indoc! {r#"
            repos:
              - repo: https://github.com/pre-commit/pre-commit-hooks
                rev: v5.0.0
                hooks:
                  - id: trailing-whitespace
                    name: 去除行尾空格
                  - id: end-of-file-fixer
                  - id: check-json
            "#
        })?;

    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: true
    exit_code: 0
    ----- stdout -----
    去除行尾空格.........................................(no files to check)Skipped
    fix end of files.....................................(no files to check)Skipped
    check json...........................................(no files to check)Skipped

    ----- stderr -----
    "#);

    Ok(())
}

// TODO: test `skips`
// TODO: test `files` and `exclude`
// TODO: test `types`, `types_or`, `exclude_types`
