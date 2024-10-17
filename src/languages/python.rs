use crate::languages::Language;

pub struct Python;

impl Language for Python {
    fn name(&self) -> &str {
        "Python"
    }

    fn environment_dir(&self) -> Option<&str> {
        Some("py-env")
    }

    fn install(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn run(&self) -> anyhow::Result<()> {
        todo!()
    }
}
