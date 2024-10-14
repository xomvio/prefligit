use crate::languages::Language;

pub struct Python;

impl Language for Python {
    fn name(&self) -> &str {
        "Python"
    }

    fn default_version(&self) -> &str {
        todo!()
    }

    fn need_install(&self) -> bool {
        todo!()
    }

    fn env_dir(&self) -> &str {
        "py-env"
    }

    fn install(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn run(&self) -> anyhow::Result<()> {
        todo!()
    }
}
