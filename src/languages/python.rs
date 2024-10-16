use crate::languages::Language;

pub struct Python;

impl Language for Python {
    fn name(&self) -> &str {
        "Python"
    }

    fn need_install(&self) -> bool {
        todo!()
    }

    fn environment_dir(&self) -> &str {
        "py-env"
    }

    fn install(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn run(&self) -> anyhow::Result<()> {
        todo!()
    }
}
