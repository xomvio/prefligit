use std::collections::HashMap;
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};
use tracing::warn;
use url::Url;

pub const CONFIG_FILE: &str = ".pre-commit-config.yaml";
pub const MANIFEST_FILE: &str = ".pre-commit-hooks.yaml";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
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

impl Language {
    pub fn default_version(&self) -> Option<String> {
        None
    }

    pub fn need_env(&self) -> bool {
        match self {
            Self::Python => true,
            Self::Node => true,
            Self::System => false,
            _ => false,
        }
    }
}

impl Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Conda => "conda",
            Self::Coursier => "coursier",
            Self::Dart => "dart",
            Self::Docker => "docker",
            Self::DockerImage => "docker-image",
            Self::Dotnet => "dotnet",
            Self::Fail => "fail",
            Self::Golang => "golang",
            Self::Haskell => "haskell",
            Self::Lua => "lua",
            Self::Node => "node",
            Self::Perl => "perl",
            Self::Python => "python",
            Self::R => "r",
            Self::Ruby => "ruby",
            Self::Rust => "rust",
            Self::Swift => "swift",
            Self::Pygrep => "pygrep",
            Self::Script => "script",
            Self::System => "system",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
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

// TODO: warn on deprecated stages
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Manual,
    CommitMsg,
    PostCheckout,
    PostCommit,
    PostMerge,
    PostRewrite,
    #[serde(alias = "commit")]
    PreCommit,
    #[serde(alias = "merge-commit")]
    PreMergeCommit,
    #[serde(alias = "push")]
    PrePush,
    PreRebase,
    PrepareCommitMsg,
}

// TODO: warn unexpected keys
// TODO: warn deprecated stage
// TODO: warn sensible regex
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigWire {
    pub repos: Vec<ConfigRepo>,
    pub default_install_hook_types: Option<Vec<HookType>>,
    pub default_language_version: Option<HashMap<Language, String>>,
    pub default_stages: Option<Vec<Stage>>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub fail_fast: Option<bool>,
    pub minimum_pre_commit_version: Option<String>,
    /// Configuration for pre-commit.ci service.
    pub ci: Option<HashMap<String, serde_yaml::Value>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoLocation {
    Local,
    Meta,
    Remote(Url),
}

impl FromStr for RepoLocation {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(RepoLocation::Local),
            "meta" => Ok(RepoLocation::Meta),
            _ => Url::parse(s).map(RepoLocation::Remote),
        }
    }
}

impl<'de> Deserialize<'de> for RepoLocation {
    fn deserialize<D>(deserializer: D) -> Result<RepoLocation, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RepoLocation::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl Display for RepoLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoLocation::Local => write!(f, "local"),
            RepoLocation::Meta => write!(f, "meta"),
            RepoLocation::Remote(url) => write!(f, "{}", url),
        }
    }
}

fn deserialize_option_vec<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v: Option<Vec<T>> = Option::deserialize(deserializer)?;
    match v {
        Some(v) if v.is_empty() => Ok(None),
        _ => Ok(v),
    }
}

/// A remote hook in the configuration file.
///
/// All keys in manifest hook dict are valid in a config hook dict, but are optional.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigRemoteHook {
    pub id: String,
    pub name: Option<String>,
    pub entry: Option<String>,
    pub language: Option<Language>,
    pub alias: Option<String>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub types: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub types_or: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub exclude_types: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub additional_dependencies: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub args: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub fail_fast: Option<bool>,
    pub pass_filenames: Option<bool>,
    pub description: Option<String>,
    pub language_version: Option<String>,
    pub log_file: Option<String>,
    pub require_serial: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub stages: Option<Vec<Stage>>,
    pub verbose: Option<bool>,
    pub minimum_pre_commit_version: Option<String>,
}

/// A local hook in the configuration file.
///
/// It's the same as the manifest hook definition.
pub type ConfigLocalHook = ManifestHook;

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaHookID {
    CheckHooksApply,
    CheckUselessExcludes,
    Identify,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct ConfigMetaHook {
    pub id: MetaHookID,
    // only "system" is allowed
    pub language: Option<Language>,
    // TODO: entry is not allowed
}

#[derive(Debug, Clone)]
pub struct ConfigRemoteRepo {
    pub repo: Url,
    pub rev: String,
    pub hooks: Vec<ConfigRemoteHook>,
}

#[derive(Debug, Clone)]
pub struct ConfigLocalRepo {
    pub repo: String,
    pub hooks: Vec<ConfigLocalHook>,
}

#[derive(Debug, Clone)]
pub struct ConfigMetaRepo {
    pub repo: String,
    pub hooks: Vec<ConfigMetaHook>,
}

#[derive(Debug, Clone)]
pub enum ConfigRepo {
    Remote(ConfigRemoteRepo),
    Local(ConfigLocalRepo),
    Meta(ConfigMetaRepo),
}

impl<'de> Deserialize<'de> for ConfigRepo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RepoWire {
            repo: RepoLocation,
            #[serde(flatten)]
            rest: serde_yaml::Value,
        }

        let RepoWire { repo, rest } = RepoWire::deserialize(deserializer)?;

        match repo {
            RepoLocation::Remote(url) => {
                #[derive(Deserialize)]
                struct RemoteRepo {
                    rev: String,
                    hooks: Vec<ConfigRemoteHook>,
                }
                let RemoteRepo { rev, hooks } = RemoteRepo::deserialize(rest)
                    .map_err(|e| serde::de::Error::custom(format!("Invalid remote repo: {}", e)))?;

                Ok(ConfigRepo::Remote(ConfigRemoteRepo {
                    repo: url,
                    rev,
                    hooks,
                }))
            }
            RepoLocation::Local => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct LocalRepo {
                    hooks: Vec<ConfigLocalHook>,
                }
                let LocalRepo { hooks } = LocalRepo::deserialize(rest)
                    .map_err(|e| serde::de::Error::custom(format!("Invalid local repo: {}", e)))?;
                Ok(ConfigRepo::Local(ConfigLocalRepo {
                    repo: "local".to_string(),
                    hooks,
                }))
            }
            RepoLocation::Meta => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct MetaRepo {
                    hooks: Vec<ConfigMetaHook>,
                }
                let MetaRepo { hooks } = MetaRepo::deserialize(rest)
                    .map_err(|e| serde::de::Error::custom(format!("Invalid meta repo: {}", e)))?;
                Ok(ConfigRepo::Meta(ConfigMetaRepo {
                    repo: "meta".to_string(),
                    hooks,
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ManifestHook {
    pub id: String,
    pub name: String,
    pub entry: String,
    pub language: Language,
    pub alias: Option<String>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub types: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub types_or: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub exclude_types: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub additional_dependencies: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub args: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub fail_fast: Option<bool>,
    pub pass_filenames: Option<bool>,
    pub description: Option<String>,
    pub language_version: Option<String>,
    pub log_file: Option<String>,
    pub require_serial: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_option_vec")]
    pub stages: Option<Vec<Stage>>,
    pub verbose: Option<bool>,
    pub minimum_pre_commit_version: Option<String>,
}

impl ManifestHook {
    pub fn update(&mut self, repo_hook: ConfigRemoteHook) {
        self.alias = repo_hook.alias;

        if let Some(name) = repo_hook.name {
            self.name = name;
        }
        if repo_hook.language_version.is_some() {
            self.language_version = repo_hook.language_version;
        }
        if repo_hook.files.is_some() {
            self.files = repo_hook.files;
        }
        if repo_hook.exclude.is_some() {
            self.exclude = repo_hook.exclude;
        }
        if repo_hook.types.is_some() {
            self.types = repo_hook.types;
        }
        if repo_hook.types_or.is_some() {
            self.types_or = repo_hook.types_or;
        }
        if repo_hook.exclude_types.is_some() {
            self.exclude_types = repo_hook.exclude_types;
        }
        if repo_hook.args.is_some() {
            self.args = repo_hook.args;
        }
        if repo_hook.stages.is_some() {
            self.stages = repo_hook.stages;
        }
        if repo_hook.additional_dependencies.is_some() {
            self.additional_dependencies = repo_hook.additional_dependencies;
        }
        if repo_hook.always_run.is_some() {
            self.always_run = repo_hook.always_run;
        }
        if repo_hook.verbose.is_some() {
            self.verbose = repo_hook.verbose;
        }
        if repo_hook.log_file.is_some() {
            self.log_file = repo_hook.log_file;
        }
    }

    pub fn fill(&mut self, config: &ConfigWire) {
        let language = self.language;
        if self.language_version.is_none() {
            self.language_version = config
                .default_language_version
                .as_ref()
                .and_then(|v| v.get(&language).cloned())
        }
        if self.language_version.is_none() {
            self.language_version = language.default_version();
        }

        if self.stages.is_none() {
            self.stages = config.default_stages.clone();
        }

        // TODO: check ENVIRONMENT_DIR with language_version and additional_dependencies
        if !language.need_env() {
            if self.language_version.is_some() {
                warn!(
                    "Language {} does not need environment, but language_version is set",
                    language
                );
            }

            if self.additional_dependencies.is_some() {
                warn!(
                    "Language {} does not need environment, but additional_dependencies is set",
                    language
                );
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(transparent)]
pub struct ManifestWire {
    pub hooks: Vec<ManifestHook>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("Failed to parse: `{0}`")]
    Yaml(String, #[source] serde_yaml::Error),

    #[error("Invalid repo URL: {0}")]
    RepoUrl(#[from] url::ParseError),
}

/// Read the configuration file from the given path.
pub fn read_config(path: &Path) -> Result<ConfigWire, Error> {
    let content = fs_err::read_to_string(path)?;
    let config =
        serde_yaml::from_str(&content).map_err(|e| Error::Yaml(path.display().to_string(), e))?;
    Ok(config)
}

// TODO: check id duplication?
/// Read the manifest file from the given path.
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
    fn parse_repos() {
        // Local hook should not have `rev`
        let yaml = indoc::indoc! {r#"
            repos:
              - repo: local
                hooks:
                  - id: cargo-fmt
                    name: cargo fmt
                    entry: cargo fmt --
                    language: system
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Ok(
            ConfigWire {
                repos: [
                    Local(
                        ConfigLocalRepo {
                            repo: "local",
                            hooks: [
                                ManifestHook {
                                    id: "cargo-fmt",
                                    name: "cargo fmt",
                                    entry: "cargo fmt --",
                                    language: System,
                                    alias: None,
                                    files: None,
                                    exclude: None,
                                    types: None,
                                    types_or: None,
                                    exclude_types: None,
                                    additional_dependencies: None,
                                    args: None,
                                    always_run: None,
                                    fail_fast: None,
                                    pass_filenames: None,
                                    description: None,
                                    language_version: None,
                                    log_file: None,
                                    require_serial: None,
                                    stages: None,
                                    verbose: None,
                                    minimum_pre_commit_version: None,
                                },
                            ],
                        },
                    ),
                ],
                default_install_hook_types: None,
                default_language_version: None,
                default_stages: None,
                files: None,
                exclude: None,
                fail_fast: None,
                minimum_pre_commit_version: None,
                ci: None,
            },
        )
        "###);

        let yaml = indoc::indoc! {r#"
            repos:
              - repo: local
                rev: v1.0.0
                hooks:
                  - id: cargo-fmt
                    name: cargo fmt
                    types:
                      - rust
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Err(
            Error("repos: Invalid local repo: unknown field `rev`, expected `hooks`", line: 2, column: 3),
        )
        "###);

        // Remote hook should have `rev`.
        let yaml = indoc::indoc! {r#"
            repos:
              - repo: https://github.com/crate-ci/typos
                rev: v1.0.0
                hooks:
                  - id: typos
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Ok(
            ConfigWire {
                repos: [
                    Remote(
                        ConfigRemoteRepo {
                            repo: Url {
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
                            rev: "v1.0.0",
                            hooks: [
                                ConfigRemoteHook {
                                    id: "typos",
                                    name: None,
                                    entry: None,
                                    language: None,
                                    alias: None,
                                    files: None,
                                    exclude: None,
                                    types: None,
                                    types_or: None,
                                    exclude_types: None,
                                    additional_dependencies: None,
                                    args: None,
                                    always_run: None,
                                    fail_fast: None,
                                    pass_filenames: None,
                                    description: None,
                                    language_version: None,
                                    log_file: None,
                                    require_serial: None,
                                    stages: None,
                                    verbose: None,
                                    minimum_pre_commit_version: None,
                                },
                            ],
                        },
                    ),
                ],
                default_install_hook_types: None,
                default_language_version: None,
                default_stages: None,
                files: None,
                exclude: None,
                fail_fast: None,
                minimum_pre_commit_version: None,
                ci: None,
            },
        )
        "###);

        let yaml = indoc::indoc! {r#"
            repos:
              - repo: https://github.com/crate-ci/typos
                hooks:
                  - id: typos
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Err(
            Error("repos: Invalid remote repo: missing field `rev`", line: 2, column: 3),
        )
        "###);
    }

    #[test]
    fn parse_hooks() {
        // Remote hook only `id` is required.
        let yaml = indoc::indoc! { r#"
            repos:
              - repo: https://github.com/crate-ci/typos
                rev: v1.0.0
                hooks:
                  - name: typos
                    alias: typo
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Err(
            Error("repos: Invalid remote repo: missing field `id`", line: 2, column: 3),
        )
        "###);

        // Local hook should have `id`, `name`, and `entry` and `language`.
        let yaml = indoc::indoc! { r#"
            repos:
              - repo: local
                hooks:
                  - id: cargo-fmt
                    name: cargo fmt
                    entry: cargo fmt
                    types:
                      - rust
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Err(
            Error("repos: Invalid local repo: missing field `language`", line: 2, column: 3),
        )
        "###);

        let yaml = indoc::indoc! { r#"
            repos:
              - repo: local
                hooks:
                  - id: cargo-fmt
                    name: cargo fmt
                    entry: cargo fmt
                    language: rust
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result, @r###"
        Ok(
            ConfigWire {
                repos: [
                    Local(
                        ConfigLocalRepo {
                            repo: "local",
                            hooks: [
                                ManifestHook {
                                    id: "cargo-fmt",
                                    name: "cargo fmt",
                                    entry: "cargo fmt",
                                    language: Rust,
                                    alias: None,
                                    files: None,
                                    exclude: None,
                                    types: None,
                                    types_or: None,
                                    exclude_types: None,
                                    additional_dependencies: None,
                                    args: None,
                                    always_run: None,
                                    fail_fast: None,
                                    pass_filenames: None,
                                    description: None,
                                    language_version: None,
                                    log_file: None,
                                    require_serial: None,
                                    stages: None,
                                    verbose: None,
                                    minimum_pre_commit_version: None,
                                },
                            ],
                        },
                    ),
                ],
                default_install_hook_types: None,
                default_language_version: None,
                default_stages: None,
                files: None,
                exclude: None,
                fail_fast: None,
                minimum_pre_commit_version: None,
                ci: None,
            },
        )
        "###);
    }

    #[test]
    fn test_read_config() -> Result<()> {
        let config = read_config(Path::new("tests/files/uv-pre-commit-config.yaml"))?;
        insta::assert_debug_snapshot!(config);
        Ok(())
    }

    #[test]
    fn test_read_manifest() -> Result<()> {
        let manifest = read_manifest(Path::new("tests/files/uv-pre-commit-hooks.yaml"))?;
        insta::assert_debug_snapshot!(manifest);
        Ok(())
    }
}
