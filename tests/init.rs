use predicates::prelude::*;
use std::fs;

mod common;

use crate::common::Sandbox;

#[test]
fn first_init_creates_config_dir_files() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    assert!(sb.db_path().exists());
    assert!(sb.config_path().exists());
    assert!(sb.theme_path().exists());
}

#[test]
fn init_with_set_to_nonexistent_path_fails() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    assert!(sb.db_path().exists());

    let new_path = sb.path().join("new_path");
    sb.command()
        .args(["init", "--set", new_path.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn init_with_set_to_existing_dir_succeeds() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    assert!(sb.db_path().exists());

    let new_path = sb.path().join("new_path");
    fs::create_dir_all(&new_path).unwrap();

    // Set new path
    sb.command()
        .args(["init", "--set", new_path.to_str().unwrap()])
        .assert()
        .success();

    // Confirm where -d reflects new path
    sb.command()
        .args(["where", "-d"])
        .assert()
        .success()
        .stdout(predicate::eq(new_path.join("checklist.sqlite").display().to_string()).trim());
}

#[test]
fn init_with_set_to_existing_sqlite_succeeds() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    assert!(sb.db_path().exists());

    let new_path = sb.path().join("new_path");
    fs::create_dir_all(&new_path).unwrap();
    fs::write(new_path.join("somefile.sqlite"), b"Just needs to exist").unwrap();

    // Set new path
    sb.command()
        .args([
            "init",
            "--set",
            new_path.join("somefile.sqlite").to_str().unwrap(),
        ])
        .assert()
        .success();

    // Confirm where -d reflects new path
    sb.command()
        .args(["where", "-d"])
        .assert()
        .success()
        .stdout(predicate::eq(new_path.join("somefile.sqlite").display().to_string()).trim());
}
