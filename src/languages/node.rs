use crate::languages::Language;

pub struct Node;

impl Language for Node {
    fn name(&self) -> &str {
        "Node"
    }

    fn environment_dir(&self) -> Option<&str> {
        Some("node-env")
    }

    fn install(&self) -> anyhow::Result<()> {
        todo!()
    }

    fn run(&self) -> anyhow::Result<()> {
        todo!()
    }
}
