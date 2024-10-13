use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

pub const CONFIG_FILE: &str = ".pre-commit-config.yaml";
pub const MANIFEST_FILE: &str = ".pre-commit-hooks.yaml";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize)]
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

impl Language {
    pub fn default_version(&self) -> String {
        match self {
            Self::Python => "python3".to_string(),
            _ => "latest".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, clap::ValueEnum)]
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

// TODO: warn on deprecated stages
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
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

impl std::fmt::Display for RepoLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoLocation::Local => write!(f, "local"),
            RepoLocation::Meta => write!(f, "meta"),
            RepoLocation::Remote(url) => write!(f, "{}", url),
        }
    }
}

/// A remote hook in the configuration file.
///
/// All keys in manifest hook dict are valid in a config hook dict, but are optional.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigRemoteHook {
    pub id: String,
    pub name: Option<String>,
    pub entry: Option<String>,
    pub language: Option<Language>,
    pub alias: Option<String>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub types: Option<Vec<String>>,
    pub types_or: Option<Vec<String>>,
    pub exclude_types: Option<Vec<String>>,
    pub additional_dependencies: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub fail_fast: Option<bool>,
    pub pass_filenames: Option<bool>,
    pub description: Option<String>,
    pub language_version: Option<String>,
    pub log_file: Option<String>,
    pub require_serial: Option<bool>,
    pub stages: Option<Vec<Stage>>,
    pub verbose: Option<bool>,
    pub minimum_pre_commit_version: Option<String>,
}

/// A local hook in the configuration file.
///
/// It's the same as the manifest hook definition.
pub type ConfigLocalHook = ManifestHook;

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetaHookID {
    CheckHooksApply,
    CheckUselessExcludes,
    Identify,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
pub struct ManifestHook {
    pub id: String,
    pub name: String,
    pub entry: String,
    pub language: Language,
    pub alias: Option<String>,
    pub files: Option<String>,
    pub exclude: Option<String>,
    pub types: Option<Vec<String>>,
    pub types_or: Option<Vec<String>>,
    pub exclude_types: Option<Vec<String>>,
    pub additional_dependencies: Option<Vec<String>>,
    pub args: Option<Vec<String>>,
    pub always_run: Option<bool>,
    pub fail_fast: Option<bool>,
    pub pass_filenames: Option<bool>,
    pub description: Option<String>,
    pub language_version: Option<String>,
    pub log_file: Option<String>,
    pub require_serial: Option<bool>,
    pub stages: Option<Vec<Stage>>,
    pub verbose: Option<bool>,
    pub minimum_pre_commit_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    fn parse_repos() {
        // Local hook should not have `rev`
        let yaml = indoc::indoc! {r#"
            repos:
              - repo: local
                hooks:
                  - id: cargo-fmt
                    name: cargo fmt
                    types:
                      - rust
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result);

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
        insta::assert_debug_snapshot!(result);

        // Remote hook should have `rev`.
        let yaml = indoc::indoc! {r#"
            repos:
              - repo: https://github.com/crate-ci/typos
                rev: v1.0.0
                hooks:
                  - id: typos
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result);

        let yaml = indoc::indoc! {r#"
            repos:
              - repo: https://github.com/crate-ci/typos
                hooks:
                  - id: typos
        "#};
        let result = serde_yaml::from_str::<ConfigWire>(yaml);
        insta::assert_debug_snapshot!(result);
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
        insta::assert_debug_snapshot!(result);

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
        insta::assert_debug_snapshot!(result);

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
        insta::assert_debug_snapshot!(result);
    }

    #[test]
    fn test_read_config() -> Result<()> {
        let config = read_config(Path::new("tests/data/uv-pre-commit-config.yaml"))?;
        insta::assert_debug_snapshot!(config);
        Ok(())
    }

    #[test]
    fn test_read_manifest() -> Result<()> {
        let manifest = read_manifest(Path::new("tests/data/uv-pre-commit-hooks.yaml"))?;
        insta::assert_debug_snapshot!(manifest);
        Ok(())
    }
}
