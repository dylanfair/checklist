mod common;

use predicates::prelude::*;

use crate::common::Sandbox;

#[test]
fn test_where_returns_correct_paths() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();

    sb.command()
        .arg("where")
        .assert()
        .stdout(predicate::eq(sb.path().display().to_string()).trim());

    sb.command()
        .args(["where", "-d"])
        .assert()
        .stdout(predicate::eq(sb.db_path().display().to_string()).trim());

    sb.command()
        .args(["where", "-c"])
        .assert()
        .stdout(predicate::eq(sb.config_path().display().to_string()).trim());

    sb.command()
        .args(["where", "-t"])
        .assert()
        .stdout(predicate::eq(sb.theme_path().display().to_string()).trim());
}
