use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;

use std::path::Path;

fn checklist(config_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("checklist").unwrap();
    cmd.env("CHECKLIST_CONFIG_DIR", config_dir);
    cmd
}

#[test]
fn first_init_creates_config_dir_files() {
    let tmp = tempdir().unwrap();
    checklist(tmp.path()).arg("init").assert().success();
    assert!(tmp.path().join("checklist.sqlite").exists());
    assert!(tmp.path().join("config.json").exists());
    assert!(tmp.path().join("theme.toml").exists());
}
