use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};

pub const CONFIG_FILE: &str = ".pre-commit-config.yaml";
pub const MANIFEST_FILE: &str = ".pre-commit-hooks.yaml";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    Conda,
    Coursier,
    Dart,
    Docker,
    DockerImage,
    Dotnet,
    Fail,
    Golang,
    Haskell,
    Lua,
    Node,
    Perl,
    Python,
    R,
    Ruby,
    Rust,
    Swift,
    Pygrep,
    Script,
    System,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum HookType {
    CommitMsg,
    PostCheckout,
    PostCommit,
    PostMerge,
    PostRewrite,
    #[default]
    PreCommit,
    PreMergeCommit,
    PrePush,
    PreRebase,
    PrepareCommitMsg,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    Manual,
    CommitMsg,
    PostCheckout,
    PostCommit,
    PostMerge,
    PostRewrite,
    PreCommit,
    PreMergeCommit,
    PrePush,
    PreRebase,
    PrepareCommitMsg,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigWire {
    pub repos: Vec<RepoWire>,
    pub default_install_hook_types: Option<Vec<HookType>>,
    pub default_language_version: Option<HashMap<Language, String>>,
    pub default_stages: Option<Vec<Stage>>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub fail_fast: Option<bool>,
    pub minimum_pre_commit_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Repo {
    Local,
    Meta,
    Remote(url::Url),
}

impl std::fmt::Display for Repo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Repo::Local => write!(f, "local"),
            Repo::Meta => write!(f, "meta"),
            Repo::Remote(url) => write!(f, "{}", url),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct RepoWire {
    pub repo: Repo,
    pub rev: Option<String>,
    pub hooks: Vec<HookWire>,
}

impl<'de> Deserialize<'de> for RepoWire {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RepoWireInner {
            repo: Repo,
            rev: Option<String>,
            hooks: Vec<HookWire>,
        }
        let RepoWireInner { repo, rev, hooks } = RepoWireInner::deserialize(deserializer)?;
        if matches!(repo, Repo::Remote(_)) && rev.is_none() {
            return Err(serde::de::Error::custom("rev is required for remote repos"));
        };
        Ok(RepoWire { repo, rev, hooks })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HookWire {
    pub id: String,
    pub alias: Option<String>,
    pub name: Option<String>,
    pub language_version: Option<HashMap<Language, String>>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub types: Option<Vec<String>>,
    pub types_or: Option<Vec<String>>,
    pub exclude_types: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub stages: Option<Vec<Stage>>,
    pub additional_dependencies: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub verbose: Option<bool>,
    pub log_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ManifestHook {
    pub id: String,
    pub name: String,
    pub entry: String,
    pub language: Language,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub types: Option<Vec<String>>,
    pub types_or: Option<Vec<String>>,
    pub exclude_types: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub fail_fast: Option<bool>,
    pub verbose: Option<bool>,
    pub pass_filenames: Option<bool>,
    pub require_serial: Option<bool>,
    pub description: Option<String>,
    pub language_version: Option<String>,
    pub minimum_pre_commit_version: Option<String>,
    pub args: Option<Vec<String>>,
    pub stages: Option<Vec<Stage>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ManifestWire {
    pub hooks: Vec<ManifestHook>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Failed to parse: `{0}`")]
    Yaml(String, #[source] serde_yaml::Error),
}

pub fn read_config(path: &Path) -> Result<ConfigWire, Error> {
    let content = fs_err::read_to_string(path)?;
    let config =
        serde_yaml::from_str(&content).map_err(|e| Error::Yaml(path.display().to_string(), e))?;
    Ok(config)
}

pub fn read_manifest(path: &Path) -> Result<ManifestWire, Error> {
    let content = fs_err::read_to_string(path)?;
    let manifest =
        serde_yaml::from_str(&content).map_err(|e| Error::Yaml(path.display().to_string(), e))?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_rev() -> Result<()> {
        let yaml = indoc::indoc! {r#"
            repos:
              - repo: local
                hooks:
                  - id: cargo-fmt
                    name: cargo fmt
                    types:
                      - rust
        "#};
        let config: ConfigWire = serde_yaml::from_str(yaml)?;
        insta::assert_debug_snapshot!(config, @r###"
        ConfigWire {
            repos: [
                RepoWire {
                    repo: Local,
                    rev: None,
                    hooks: [
                        HookWire {
                            id: "cargo-fmt",
                            alias: None,
                            name: Some(
                                "cargo fmt",
                            ),
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: Some(
                                [
                                    "rust",
                                ],
                            ),
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
            ],
            default_install_hook_types: None,
            default_language_version: None,
            default_stages: None,
            files: None,
            exclude: None,
            fail_fast: None,
            minimum_pre_commit_version: None,
        }
        "###);

        let yaml = indoc::indoc! {r#"
            repos:
              - repo: https://github.com/crate-ci/typos
                hooks:
                  - id: typos
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("rev is required for remote repos"));

        Ok(())
    }

    #[test]
    fn test_read_config() -> Result<()> {
        let config = read_config(Path::new("tests/data/uv-pre-commit-config.yaml"))?;
        insta::assert_debug_snapshot!(config, @r###"
        ConfigWire {
            repos: [
                RepoWire {
                    repo: Remote(
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "github.com",
                                ),
                            ),
                            port: None,
                            path: "/abravalheri/validate-pyproject",
                            query: None,
                            fragment: None,
                        },
                    ),
                    rev: Some(
                        "v0.20.2",
                    ),
                    hooks: [
                        HookWire {
                            id: "validate-pyproject",
                            alias: None,
                            name: None,
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: None,
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
                RepoWire {
                    repo: Remote(
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "github.com",
                                ),
                            ),
                            port: None,
                            path: "/crate-ci/typos",
                            query: None,
                            fragment: None,
                        },
                    ),
                    rev: Some(
                        "v1.24.6",
                    ),
                    hooks: [
                        HookWire {
                            id: "typos",
                            alias: None,
                            name: None,
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: None,
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
                RepoWire {
                    repo: Local,
                    rev: None,
                    hooks: [
                        HookWire {
                            id: "cargo-fmt",
                            alias: None,
                            name: Some(
                                "cargo fmt",
                            ),
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: Some(
                                [
                                    "rust",
                                ],
                            ),
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
                RepoWire {
                    repo: Local,
                    rev: None,
                    hooks: [
                        HookWire {
                            id: "cargo-dev-generate-all",
                            alias: None,
                            name: Some(
                                "cargo dev generate-all",
                            ),
                            language_version: None,
                            files: Some(
                                "^crates/(uv-cli|uv-settings)/",
                            ),
                            exclude: None,
                            types: Some(
                                [
                                    "rust",
                                ],
                            ),
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
                RepoWire {
                    repo: Remote(
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "github.com",
                                ),
                            ),
                            port: None,
                            path: "/pre-commit/mirrors-prettier",
                            query: None,
                            fragment: None,
                        },
                    ),
                    rev: Some(
                        "v3.1.0",
                    ),
                    hooks: [
                        HookWire {
                            id: "prettier",
                            alias: None,
                            name: None,
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: None,
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
                RepoWire {
                    repo: Remote(
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "github.com",
                                ),
                            ),
                            port: None,
                            path: "/astral-sh/ruff-pre-commit",
                            query: None,
                            fragment: None,
                        },
                    ),
                    rev: Some(
                        "v0.6.8",
                    ),
                    hooks: [
                        HookWire {
                            id: "ruff-format",
                            alias: None,
                            name: None,
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: None,
                            types_or: None,
                            exclude_types: None,
                            args: None,
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                        HookWire {
                            id: "ruff",
                            alias: None,
                            name: None,
                            language_version: None,
                            files: None,
                            exclude: None,
                            types: None,
                            types_or: None,
                            exclude_types: None,
                            args: Some(
                                [
                                    "--fix",
                                    "--exit-non-zero-on-fix",
                                ],
                            ),
                            stages: None,
                            additional_dependencies: None,
                            always_run: None,
                            verbose: None,
                            log_file: None,
                        },
                    ],
                },
            ],
            default_install_hook_types: None,
            default_language_version: None,
            default_stages: None,
            files: None,
            exclude: Some(
                "(?x)^(\n  .*/(snapshots)/.*|\n)$\n",
            ),
            fail_fast: None,
            minimum_pre_commit_version: None,
        }
        "###);

        Ok(())
    }
}
