mod node;
mod python;
mod system;

use anyhow::Result;

use crate::config::Language;
use crate::hook::Hook;

pub const DEFAULT_VERSION: &str = "default";

macro_rules! delegate_to_language {
    ( $lang:ident, $func:ident $(, $param:ident )* ) => {
        match $lang {
            // Self::Conda => crate::languages::Conda.$func($($param),*),
            // Self::Coursier => crate::languages::Coursier.$func($($param),*),
            // Self::Dart => crate::languages::Dart.$func($($param),*),
            // Self::Docker => crate::languages::Docker.$func($($param),*),
            // Self::DockerImage => crate::languages::DockerImage.$func($($param),*),
            // Self::Dotnet => crate::languages::Dotnet.$func($($param),*),
            // Self::Fail => crate::languages::Fail.$func($($param),*),
            // Self::Golang => crate::languages::Golang.$func($($param),*),
            // Self::Haskell => crate::languages::Haskell.$func($($param),*),
            // Self::Lua => crate::languages::Lua.$func($($param),*),
            Language::Node => node::Node.$func($($param),*),
            // Self::Perl => crate::languages::Perl.$func($($param),*),
            Language::Python => python::Python.$func($($param),*),
            // Self::R => crate::languages::R.$func($($param),*),
            // Self::Ruby => crate::languages::Ruby.$func($($param),*),
            // Self::Rust => crate::languages::Rust.$func($($param),*),
            // Self::Swift => crate::languages::Swift.$func($($param),*),
            // Self::Pygrep => crate::languages::Pygrep.$func($($param),*),
            // Self::Script => crate::languages::Script.$func($($param),*),
            Language::System => system::System.$func($($param),*),
            _ => unimplemented!(),
        }
    };
    ( $lang:ident, async $func:ident $(, $param:ident )* ) => {
        match $lang {
            // Self::Conda => crate::languages::Conda.$func($($param),*),
            // Self::Coursier => crate::languages::Coursier.$func($($param),*),
            // Self::Dart => crate::languages::Dart.$func($($param),*),
            // Self::Docker => crate::languages::Docker.$func($($param),*),
            // Self::DockerImage => crate::languages::DockerImage.$func($($param),*),
            // Self::Dotnet => crate::languages::Dotnet.$func($($param),*),
            // Self::Fail => crate::languages::Fail.$func($($param),*),
            // Self::Golang => crate::languages::Golang.$func($($param),*),
            // Self::Haskell => crate::languages::Haskell.$func($($param),*),
            // Self::Lua => crate::languages::Lua.$func($($param),*),
            Language::Node => node::Node.$func($($param),*).await,
            // Self::Perl => crate::languages::Perl.$func($($param),*),
            Language::Python => python::Python.$func($($param),*).await,
            // Self::R => crate::languages::R.$func($($param),*),
            // Self::Ruby => crate::languages::Ruby.$func($($param),*),
            // Self::Rust => crate::languages::Rust.$func($($param),*),
            // Self::Swift => crate::languages::Swift.$func($($param),*),
            // Self::Pygrep => crate::languages::Pygrep.$func($($param),*),
            // Self::Script => crate::languages::Script.$func($($param),*),
            Language::System => system::System.$func($($param),*).await,
            _ => unimplemented!(),
        }
    }
}

impl Language {
    pub fn name(&self) -> &str {
        delegate_to_language!(self, name)
    }

    pub fn default_version(&self) -> &str {
        delegate_to_language!(self, default_version)
    }

    pub fn environment_dir(&self) -> Option<&str> {
        delegate_to_language!(self, environment_dir)
    }

    pub async fn install(&self, hook: &Hook) -> Result<()> {
        delegate_to_language!(self, async install, hook)
    }

    pub async fn run(&self, hook: &Hook) -> Result<()> {
        delegate_to_language!(self, async run, hook)
    }
}
