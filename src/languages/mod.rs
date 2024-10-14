mod python;

use anyhow::Result;

use crate::config;
use crate::languages::python::Python;

pub trait Language {
    fn name(&self) -> &str;
    fn default_version(&self) -> &str;
    fn need_install(&self) -> bool;
    fn env_dir(&self) -> &str;
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
