use anyhow::Result;
use assert_cmd::Command;
use assert_fs::prelude::*;
use crate::common::{cmd_snapshot, TestContext};

mod common;

#[test]
fn run_basic() -> Result<()> {
    let context = TestContext::new();

    context
        .workdir()
        .child(".pre-commit-config.yaml")
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
    context.workdir().child("file.txt").write_str("Hello, world!\n")?;
    context.workdir().child("valid.json").write_str("{}")?;
    context.workdir().child("invalid.json").write_str("{}")?;
    context.workdir().child("main.py").write_str(r#"print "abc"  "#)?;
    Command::new("git")
        .current_dir(context.workdir())
        .arg("add")
        .arg(".")
        .assert()
        .success();

    cmd_snapshot!(context.filters(), context.run(), @r#"
    success: false
    exit_code: 2
    ----- stdout -----
    Cloning https://github.com/pre-commit/pre-commit-hooks@v5.0.0 into [HOME]/repoheycBG
    Installing environment for https://github.com/pre-commit/pre-commit-hooks@v5.0.0
    Running hook trailing-whitespace
    Running hook end-of-file-fixer
    Running hook check-json

    ----- stderr -----
    Hook failed: code=1
    stdout="Fixing main.py/n"
    stderr=""

    Hook failed: code=1
    stdout=```
    Fixing invalid.json
    Fixing main.py
    Fixing valid.json
    ```

    stderr=""

    Hook failed: code=1
    stdout=```
    .pre-commit-config.yaml: Failed to json decode (Expecting value: line 1 column 1 (char 0))
    file.txt: Failed to json decode (Expecting value: line 1 column 1 (char 0))
    main.py: Failed to json decode (Expecting value: line 1 column 1 (char 0))
    ```

    stderr=""

    error: Some hooks failed
    "#);

    cmd_snapshot!(context.filters(), context.run().arg("trailing-whitespace"), @r###"
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

    context
        .workdir()
        .child(".pre-commit-config.yaml")
        .write_str(indoc::indoc! {r#"
            repos:
              - repo: https://github.com/abravalheri/validate-pyproject
                rev: v0.20.2
                hooks:
                  - id: invalid-hook-id
            "#
        })?;

    cmd_snapshot!(context.filters(), context.run().arg("invalid-hook-id"), @r###"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: Hook not found: invalid-hook-id in repo https://github.com/abravalheri/validate-pyproject@v0.20.2
    "###);

    Ok(())
}
