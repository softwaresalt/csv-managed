#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::{tempdir, TempDir};

/// Returns the absolute path to a fixture under `tests/data`.
pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join(name)
}

/// Scratch directory helper that cleans up files automatically on drop.
pub struct TestWorkspace {
    temp_dir: TempDir,
}

impl TestWorkspace {
    /// Creates a fresh scratch directory for the current test case.
    pub fn new() -> Self {
        Self {
            temp_dir: tempdir().expect("temp dir"),
        }
    }

    /// Returns the root path for all files owned by this workspace.
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Writes `contents` into a file under the workspace and returns the path.
    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.temp_dir.path().join(name);
        let mut file = File::create(&path).expect("create temp file");
        file.write_all(contents.as_bytes())
            .expect("write temp file contents");
        path
    }
}