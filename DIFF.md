## Difference from pre-commit

- `prefligit` supports both `.pre-commit-config.yaml` and `.pre-commit-config.yml` configuration files.
- `prefligit` implements some common hooks from `pre-commit-hooks` in Rust for better performance.
- `prefligit` uses `~/.prefligit` as the default cache directory for toolchains and environments, and stores repos and hooks separately.
- `prefligit` uses `uv` for managing Python environments and installations.
- `prefligit` supports `language-version` as a version specifier and automatically installs the required toolchains.

### Future plans

- Built-in support for monorepos.
- Global configurations.
