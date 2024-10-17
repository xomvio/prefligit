mod node;
mod python;
mod system;

use std::ops::Deref;

use anyhow::Result;

use crate::config;

pub const DEFAULT_VERSION: &str = "default";

pub trait Language {
    fn name(&self) -> &str;
    fn default_version(&self) -> &str {
        DEFAULT_VERSION
    }
    fn need_install(&self) -> bool {
        self.environment_dir().is_some()
    }
    fn environment_dir(&self) -> Option<&str> {
        None
    }
    fn install(&self) -> Result<()>;
    fn run(&self) -> Result<()>;
}

impl Deref for config::Language {
    type Target = dyn Language;

    fn deref(&self) -> &Self::Target {
        match self {
            // Self::Conda => &crate::languages::Conda,
            // Self::Coursier => &crate::languages::Coursier,
            // Self::Dart => &crate::languages::Dart,
            // Self::Docker => &crate::languages::Docker,
            // Self::DockerImage => &crate::languages::DockerImage,
            // Self::Dotnet => &crate::languages::Dotnet,
            // Self::Fail => &crate::languages::Fail,
            // Self::Golang => &crate::languages::Golang,
            // Self::Haskell => &crate::languages::Haskell,
            // Self::Lua => &crate::languages::Lua,
            Self::Node => &node::Node,
            // Self::Perl => &crate::languages::Perl,
            Self::Python => &python::Python,
            // Self::R => &crate::languages::R,
            // Self::Ruby => &crate::languages::Ruby,
            // Self::Rust => &crate::languages::Rust,
            // Self::Swift => &crate::languages::Swift,
            // Self::Pygrep => &crate::languages::Pygrep,
            // Self::Script => &crate::languages::Script,
            Self::System => &system::System,
            _ => unimplemented!(),
        }
    }
}
