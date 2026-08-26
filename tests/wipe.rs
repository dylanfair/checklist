mod common;

use common::Sandbox;
use predicates::str::contains;

#[test]
fn wipe_y_removes_all_tasks() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Task A", "Low", "Open", None);
    common::seed_task(sb.db_path(), "Task B", "High", "Open", Some("work"));
    assert_eq!(common::task_count(sb.db_path()), 2);

    sb.command()
        .args(["wipe", "-y"])
        .assert()
        .success()
        .stdout(contains("Tasks from 'task' table deleted successfully"));

    assert_eq!(common::task_count(sb.db_path()), 0);
}

#[test]
fn wipe_hard_also_drops_the_task_table() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Task A", "Low", "Open", Some("work"));
    common::seed_task(sb.db_path(), "Task B", "Medium", "Paused", None);
    assert_eq!(common::task_count(sb.db_path()), 2);

    // Soft wipe output must NOT appear: --hard drops instead of deletes.
    sb.command()
        .args(["wipe", "-y", "--hard"])
        .assert()
        .success()
        .stdout(contains("table(s) dropped successfully"));

    // The whole table is gone, not just emptied.
    let conn = rusqlite::Connection::open(sb.db_path()).unwrap();
    let task_tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'task'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(task_tables, 0, "'task' table should have been dropped");
}

#[test]
fn wipe_without_y_answering_n_preserves_tasks() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Survivor 1", "Low", "Open", None);
    common::seed_task(sb.db_path(), "Survivor 2", "High", "Working", Some("keep"));

    sb.command()
        .arg("wipe")
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(contains("Are you sure you want to proceed with the wipe?"))
        .stdout(contains("Halting wipe"));

    assert_eq!(
        common::task_count(sb.db_path()),
        2,
        "answering 'n' must preserve all tasks"
    );
}

#[test]
fn wipe_reprompts_on_invalid_input_then_accepts_y() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Doomed", "Critical", "Open", Some("gone"));

    // Invalid answer reprompts; the following 'y' then proceeds. Multi-line
    // stdin demonstrates the follow-on-input capability end to end.
    sb.command()
        .arg("wipe")
        .write_stdin("maybe\ny\n")
        .assert()
        .success()
        .stdout(contains("You must provide either a 'y' or 'n'"))
        .stdout(contains("Success!"));

    assert_eq!(common::task_count(sb.db_path()), 0);
}

#[test]
fn wipe_soft_also_cascades_tag_rows() {
    // Seeding writes legacy-format tags (no tag table exists yet); migrating
    // to latest copies them into the relational tag table — and we verify
    // that copy EXISTS before wiping, so the cascade assertion below proves
    // deletion rather than passing vacuously on an empty table.
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Tagged", "High", "Open", Some("work;urgent"));

    sb.command()
        .arg("migrate")
        .arg("--latest")
        .assert()
        .success();

    let conn = rusqlite::Connection::open(sb.db_path()).unwrap();
    let tags_before: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT tag FROM tag ORDER BY tag")
            .expect("tag table should exist after migration");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        tags_before,
        vec!["urgent".to_string(), "work".to_string()],
        "migration should have copied the seeded tags into the tag table"
    );

    sb.command().args(["wipe", "-y"]).assert().success();

    let tag_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM tag", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tag_rows, 0, "cascade should have removed all tag rows");
}

#[test]
fn wipe_with_closed_stdin_halts_instead_of_looping() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Survivor", "Low", "Open", None);

    // Empty stdin = immediate EOF. Before the EOF guard this looped forever
    // printing the reprompt message; now it must halt and preserve tasks.
    sb.command()
        .arg("wipe")
        .write_stdin("")
        .assert()
        .success()
        .stdout(contains("No answer received; halting wipe"));

    assert_eq!(common::task_count(sb.db_path()), 1);
}
