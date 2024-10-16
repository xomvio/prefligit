use crate::languages::Language;

pub struct Node;

impl Language for Node {
    fn name(&self) -> &str {
        "Node"
    }

    fn need_install(&self) -> bool {
        true
    }

    fn environment_dir(&self) -> &str {
        "node-env"
    }

    fn install(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn run(&self) -> anyhow::Result<()> {
        todo!()
    }
}
