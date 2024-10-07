use std::collections::HashMap;
use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = ".pre-commit-config.yaml";
const HOOKS_FILE: &str = ".pre-commit-hooks.yaml";

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, clap::ValueEnum)]
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

#[derive(Debug, Clone)]
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

impl std::str::FromStr for Repo {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(Repo::Local),
            "meta" => Ok(Repo::Meta),
            _ => Ok(Repo::Remote(url::Url::parse(s)?)),
        }
    }
}

impl Serialize for Repo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            Repo::Local => serializer.serialize_str("local"),
            Repo::Meta => serializer.serialize_str("meta"),
            Repo::Remote(url) => serializer.serialize_str(&url.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for Repo {
    fn deserialize<D>(deserializer: D) -> Result<Repo, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "local" => Ok(Repo::Local),
            "meta" => Ok(Repo::Meta),
            _ => Ok(Repo::Remote(url::Url::parse(&s).map_err(serde::de::Error::custom)?)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RepoWire {
    pub repo: Repo,
    pub rev: String,
    pub hooks: Vec<HookWire>,
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
pub struct HooksWire {
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
