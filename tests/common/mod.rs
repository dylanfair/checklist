use assert_cmd::Command;
use tempfile::TempDir;

use std::path::{Path, PathBuf};

pub struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().expect("failed to create a temp dir"),
        }
    }

    pub fn command(&self) -> Command {
        let mut cmd = Command::cargo_bin("checklist").unwrap();
        cmd.env("CHECKLIST_CONFIG_DIR", self.dir.path());
        cmd
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn db_path(&self) -> PathBuf {
        self.dir.path().join("checklist.sqlite")
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir.path().join("config.json")
    }

    pub fn theme_path(&self) -> PathBuf {
        self.dir.path().join("theme.toml")
    }
}
