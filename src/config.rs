use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};

pub const CONFIG_FILE: &str = ".pre-commit-config.yaml";
pub const MANIFEST_FILE: &str = ".pre-commit-hooks.yaml";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

// TODO: warn on deprecated stages
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    Remote(url::Url),
}

impl FromStr for RepoLocation {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(RepoLocation::Local),
            "meta" => Ok(RepoLocation::Meta),
            _ => url::Url::parse(s).map(RepoLocation::Remote),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigRemoteHook {
    pub id: String,
    pub alias: Option<String>,
    pub name: Option<String>,
    pub language_version: Option<String>,
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
    #[serde(transparent)]
    pub struct ConfigLocalHook(ManifestHook);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetaHook {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigHook {
    Remote(ConfigRemoteHook),
    Local(ConfigLocalHook),
    Meta(ConfigMetaHook),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ConfigRepo {
    pub repo: RepoLocation,
    pub rev: Option<String>,
    pub hooks: Vec<ConfigHook>,
}

impl<'de> Deserialize<'de> for ConfigRepo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RepoWireInner {
            repo: RepoLocation,
            rev: Option<String>,
            hooks: Vec<ConfigHook>,
        }
        let RepoWireInner { repo, rev, hooks } = RepoWireInner::deserialize(deserializer)?;
        if matches!(repo, RepoLocation::Remote(_)) && rev.is_none() {
            return Err(serde::de::Error::custom("rev is required for remote repos"));
        };

        if matches!(repo, RepoLocation::Local) {
            if rev.is_some() {
                return Err(serde::de::Error::custom("rev is not allowed for local repos"));
            }
            for hook in &hooks {
                if hook.name.is_none() {
                    return Err(serde::de::Error::custom("name is required for local hooks"));
                }
                if hook.
            }
        };

        Ok(ConfigRepo { repo, rev, hooks })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        insta::assert_debug_snapshot!(config);

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
