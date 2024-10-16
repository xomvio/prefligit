mod node;
mod python;

use anyhow::Result;

use crate::config;
pub use crate::languages::node::Node;
pub use crate::languages::python::Python;

pub trait Language {
    fn name(&self) -> &str;
    fn default_version(&self) -> &str {
        "default"
    }
    fn need_install(&self) -> bool;
    fn environment_dir(&self) -> &str;
    fn install(&self) -> Result<()>;
    fn run(&self) -> Result<()>;
}

impl From<config::Language> for Box<dyn Language> {
    fn from(language: config::Language) -> Self {
        match language {
            config::Language::Python => Box::new(Python),
            _ => unimplemented!(),
        }
    }
}
