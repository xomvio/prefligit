#![allow(dead_code, unreachable_pub)]

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_fs::fixture::{ChildPath, PathChild};
use etcetera::BaseStrategy;

pub struct TestContext {
    temp_dir: ChildPath,

    /// Standard filters for this test context.
    filters: Vec<(String, String)>,

    // To keep the directory alive.
    #[allow(dead_code)]
    _root: tempfile::TempDir,
}

impl TestContext {
    pub fn new() -> Self {
        let bucket = Self::test_bucket_dir();
        fs_err::create_dir_all(&bucket).expect("Failed to create test bucket");

        let root = tempfile::TempDir::new_in(bucket).expect("Failed to create test root directory");

        let temp_dir = ChildPath::new(root.path()).child("temp");
        fs_err::create_dir_all(&temp_dir).expect("Failed to create test working directory");

        let mut filters = Vec::new();

        filters.push((temp_dir.display().to_string(), "[TEMP_DIR]/".to_string()));

        Self {
            temp_dir,
            filters,
            _root: root,
        }
    }

    pub fn test_bucket_dir() -> PathBuf {
        env::var("PRE_COMMIT_RS_INTERNAL__TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                etcetera::base_strategy::choose_base_strategy()
                    .expect("Failed to find base strategy")
                    .data_dir()
                    .join("pre-commit-rs")
                    .join("tests")
            })
    }

    /// Read a file in the temporary directory
    pub fn read(&self, file: impl AsRef<Path>) -> String {
        fs_err::read_to_string(self.temp_dir.join(&file))
            .unwrap_or_else(|_| panic!("Missing file: `{}`", file.as_ref().display()))
    }

    pub fn command(&self) -> Command {
        let bin = assert_cmd::cargo::cargo_bin("pre-commit-rs");
        let mut cmd = Command::new(bin);
        cmd.current_dir(self.workdir());
        cmd
    }

    pub fn run(&self) -> Command {
        let mut command = self.command();
        command.arg("compat").arg("run");
        command
    }

    /// Standard snapshot filters _plus_ those for this test context.
    pub fn filters(&self) -> Vec<(&str, &str)> {
        // Put test context snapshots before the default filters
        // This ensures we don't replace other patterns inside paths from the test context first
        self.filters
            .iter()
            .map(|(p, r)| (p.as_str(), r.as_str()))
            .chain(INSTA_FILTERS.iter().copied())
            .collect()
    }

    pub fn workdir(&self) -> &ChildPath {
        &self.temp_dir
    }
}

#[doc(hidden)] // Macro and test context only, don't use directly.
pub const INSTA_FILTERS: &[(&str, &str)] = &[
    // File sizes
    (r"(\s|\()(\d+\.)?\d+([KM]i)?B", "$1[SIZE]"),
    // Rewrite Windows output to Unix output
    (r"\\([\w\d]|\.\.)", "/$1"),
    (r"pre-commit-rs.exe", "pre-commit-rs"),
    // The exact message is host language dependent
    (
        r"Caused by: .* \(os error 2\)",
        "Caused by: No such file or directory (os error 2)",
    ),
];

#[allow(unused_macros)]
macro_rules! cmd_snapshot {
    ($spawnable:expr, @$snapshot:literal) => {{
        cmd_snapshot!($crate::common::INSTA_FILTERS.iter().copied().collect::<Vec<_>>(), $spawnable, @$snapshot)
    }};
    ($filters:expr, $spawnable:expr, @$snapshot:literal) => {{
        let mut settings = insta::Settings::clone_current();
        for (matcher, replacement) in $filters {
            settings.add_filter(matcher, replacement);
        }
        let _guard = settings.bind_to_scope();
        insta_cmd::assert_cmd_snapshot!($spawnable, @$snapshot);
    }};
}

#[allow(unused_imports)]
pub(crate) use cmd_snapshot;
