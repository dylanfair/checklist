mod common;

use common::Sandbox;
use predicates::str::contains;
use std::collections::HashSet;

/// Current `user_version` of the database — how SQLite (and
/// rusqlite_migration) track which schema version the file is at.
fn user_version(db_path: &std::path::Path) -> i64 {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap()
}

/// (name, sorted tags) for every task, read positionally from the legacy
/// `tags` column — i.e., exactly how a pre-V1 binary sees the data.
fn legacy_tasks_with_tags(db_path: &std::path::Path) -> Vec<(String, HashSet<String>)> {
    let conn = rusqlite::Connection::open(db_path).unwrap();
    let mut stmt = conn.prepare("SELECT name, tags FROM task").unwrap();
    let mut rows: Vec<(String, HashSet<String>)> = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .unwrap()
        .map(|row| {
            let (name, tags_raw) = row.unwrap();
            let tags = tags_raw
                .unwrap_or_default()
                .split(';')
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect();
            (name, tags)
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}

#[test]
fn migrate_to_1_then_prior_round_trips_the_schema() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(
        sb.db_path(),
        "Write report",
        "High",
        "Open",
        Some("work;urgent"),
    );
    common::seed_task(sb.db_path(), "Buy milk", "Low", "Open", None);

    // --to 1 is used instead of --latest so this test keeps passing when
    // future versions add more migrations (--latest would start pointing at
    // the newest one).
    sb.command()
        .args(["migrate", "--to", "1"])
        .assert()
        .success()
        .stdout(contains("up to schema version 1"))
        .stdout(contains("Backed up database")); // snapshot taken before moving

    // Version stamped at exactly 1, tags copied relationally.
    assert_eq!(user_version(&sb.db_path()), 1);
    {
        let conn = rusqlite::Connection::open(sb.db_path()).unwrap();
        let tag_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM tag", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tag_rows, 2, "both tags should have been copied");
        assert!(
            conn.prepare("SELECT tags FROM task").is_err(),
            "legacy tags column should be gone at V1"
        );
    }

    // --prior moves back down; downward moves always take a snapshot too.
    sb.command()
        .args(["migrate", "--prior", "-y"])
        .assert()
        .success()
        .stdout(contains("down to schema version 0"))
        .stdout(contains("checklist versions before v0.1.9"));

    assert_eq!(user_version(&sb.db_path()), 0);

    // Tasks survive, with tags back in the legacy column. group_concat
    // ordering is unspecified, so compare tags as sets.
    let tasks = legacy_tasks_with_tags(&sb.db_path());
    assert_eq!(
        tasks,
        vec![
            ("Buy milk".to_string(), HashSet::new()),
            (
                "Write report".to_string(),
                HashSet::from_iter(["work".to_string(), "urgent".to_string()])
            ),
        ]
    );
}

#[test]
fn migrate_latest_upgrades_cleanly_without_errors() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(
        sb.db_path(),
        "Write report",
        "High",
        "Open",
        Some("work;urgent"),
    );
    common::seed_task(sb.db_path(), "Buy milk", "Low", "Open", None);

    // Happy path: success, a pre-migration snapshot taken, and nothing at
    // all on stderr.
    sb.command()
        .args(["migrate", "--latest"])
        .assert()
        .success()
        .stdout(contains("Backed up database"))
        .stderr(predicates::str::is_empty());

    assert_eq!(user_version(&sb.db_path()), 1);

    // Data made it across: both tags live in the tag table, attached to the
    // right task.
    let conn = rusqlite::Connection::open(sb.db_path()).unwrap();
    let tagged_task_tags: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT tag FROM tag WHERE task_id = \
                 (SELECT id FROM task WHERE name = 'Write report')",
            )
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    };
    let mut tag_set = tagged_task_tags.clone();
    tag_set.sort();
    assert_eq!(tag_set, vec!["urgent".to_string(), "work".to_string()]);
}

#[test]
fn migrate_prior_answering_n_keeps_database_untouched() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();
    common::seed_task(sb.db_path(), "Stays put", "Medium", "Working", Some("keep"));

    // Get to V1 first so there is something to go "prior" from.
    sb.command()
        .args(["migrate", "--to", "1"])
        .assert()
        .success();
    let version_before = user_version(&sb.db_path());
    assert_eq!(version_before, 1);

    // Answering 'n' aborts before the snapshot or migration run, so the
    // database must be byte-for-byte semantically unchanged: same version,
    // same relational tags.
    sb.command()
        .args(["migrate", "--prior"])
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(contains("Aborted; database unchanged"));

    assert_eq!(user_version(&sb.db_path()), 1);
    let conn = rusqlite::Connection::open(sb.db_path()).unwrap();
    let tag_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM tag", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tag_rows, 1, "'keep' tag must still be attached");
}
