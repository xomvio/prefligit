use crate::languages::Language;

pub struct System;

impl Language for System {
    fn name(&self) -> &str {
        "System"
    }

    fn need_install(&self) -> bool {
        false
    }

    fn environment_dir(&self) -> &str {
        "system"
    }

    fn install(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn run(&self) -> anyhow::Result<()> {
        todo!()
    }
}
