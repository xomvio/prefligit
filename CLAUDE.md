# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

**prefligit** is a Rust reimplementation of the pre-commit tool, designed to be a faster, dependency-free drop-in alternative. It provides git hook management with improved performance and additional features like integration with `uv` for Python environment management.

## Commands

### Development Commands

- **Build**: `cargo build` or `cargo build --release`
- **Run**: `cargo run -- <args>` (e.g., `cargo run -- run`)
- **Test**:
  - All tests: `cargo test --all-targets --all-features --workspace` or `make test`
  - Unit tests: `make test-unit` or `make test-all-unit`
  - Integration tests: `make test-integration` or `make test-all-integration`
  - Single test: `cargo insta test --review --bin prefligit -- <test_filter>`
- **Linting**:
  - Format: `cargo fmt`
  - Clippy: `cargo clippy --all-targets --all-features --workspace -- -D warnings`
  - Combined: `make lint`
- **Install from source**: `cargo install --locked --path .`

### Testing Framework

This project uses `insta` for snapshot testing. When tests fail due to output changes, review with `cargo insta review` and accept changes if correct.

## Architecture

### Core Components

- **CLI Layer** (`src/cli/`): Command-line interface using `clap`, structured with subcommands for different operations
- **Configuration** (`src/config.rs`): Handles `.pre-commit-config.yaml` parsing and validation
- **Hook System** (`src/hook.rs`): Core hook definition, validation, and execution logic
- **Language Support** (`src/languages/`): Modular language implementations (Python, Node, Docker, etc.)
- **Store Management** (`src/store.rs`): Cache and repository management in `~/.prefligit`
- **Git Integration** (`src/git.rs`): Git repository operations and file change detection
- **Process Management** (`src/process.rs` & `src/run.rs`): Hook execution with concurrency control

### Key Features

- **Language Implementations**: Supports Python (with `uv` integration), Node.js, Docker, and more
- **Built-in Hooks** (`src/builtin/`): Rust implementations of common pre-commit hooks for better performance
- **Concurrent Execution**: Configurable parallel hook execution
- **Environment Management**: Automatic toolchain installation and management

### Configuration Files

- **Main config**: `.pre-commit-config.yaml` (or `.yml`)
- **Hook manifests**: `.pre-commit-hooks.yaml`
- **Rust toolchain**: Uses Rust 1.88 (see `rust-toolchain.toml`)
- **Cache directory**: `~/.prefligit` (differs from original pre-commit's `~/.cache/pre-commit`)

## Development Notes

### Clippy Configuration

The project has specific clippy rules configured in `Cargo.toml` and `clippy.toml`:
- Pedantic warnings enabled with specific allows
- Disallowed methods: `std::env::var` and `std::env::var_os` (use project's env var handling instead)

### Testing Structure

- Unit tests are co-located with source files
- Integration tests are in `tests/` directory
- Test fixtures are in `tests/fixtures/`
- Snapshots are in `src/snapshots/` and are tracked in git

### Code Organization

The codebase follows a modular structure:
- Each language has its own module in `src/languages/`
- CLI commands are implemented in `src/cli/`
- Core logic is split between configuration, hook management, and execution
- Error handling uses `anyhow` with custom error types where needed

### Key Differences from Original pre-commit

- Uses `~/.prefligit` as cache directory instead of `~/.cache/pre-commit`
- Supports both `.yaml` and `.yml` config files
- Built-in Rust implementations of common hooks
- Integration with `uv` for Python management
- Automatic toolchain installation via `language-version`

This project is in early development stage with only a subset of original pre-commit features implemented. Current supported languages are `python`, `node`, `docker`, `docker-image`, `system`, `script`, and `fail`.
