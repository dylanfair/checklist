mod common;

use common::{Sandbox, TaskSummary};

#[test]
fn import_moves_all_tasks_between_databases() {
    // 1. A database in one location holding three tasks. `init` creates the
    //    old-style schema (version 0), and seed_task writes the way the
    //    original writer did — so the source stays a genuine pre-V1 database.
    let source = Sandbox::new();
    source.command().arg("init").assert().success();
    common::seed_task(
        source.db_path(),
        "Write report",
        "High",
        "Open",
        Some("work;urgent"),
    );
    common::seed_task(source.db_path(), "Buy milk", "Low", "Open", None);
    common::seed_task(
        source.db_path(),
        "Call mom",
        "Medium",
        "Working",
        Some("family"),
    );

    // 2. All three are present in the source database.
    let seeded = common::read_tasks(source.db_path());
    assert_eq!(seeded.len(), 3);
    assert_eq!(
        seeded,
        vec![
            TaskSummary {
                name: "Buy milk".into(),
                urgency: "Low".into(),
                status: "Open".into(),
                tags: vec![],
            },
            TaskSummary {
                name: "Call mom".into(),
                urgency: "Medium".into(),
                status: "Working".into(),
                tags: vec!["family".into()],
            },
            TaskSummary {
                name: "Write report".into(),
                urgency: "High".into(),
                status: "Open".into(),
                tags: vec!["urgent".into(), "work".into()],
            },
        ]
    );

    // 3. A second database in another location starts out empty.
    let dest = Sandbox::new();
    dest.command().arg("init").assert().success();
    assert_eq!(common::task_count(dest.db_path()), 0);

    // 4. Import from the source into this sandbox's configured database.
    dest.command()
        .arg("import")
        .arg(source.db_path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Adding 3 tasks to current database",
        ));

    // 5. The same tasks — names, urgencies, statuses, and tags — exist on the
    //    other side, in the destination's current schema format.
    let imported = common::read_tasks(dest.db_path());
    assert_eq!(imported.len(), 3);
    assert_eq!(imported, seeded);
}

#[test]
fn import_handles_legacy_double_separators() {
    // Legacy ;-joined strings could contain empty segments (';;' or trailing
    // ';'). The old reader dropped them; importing from a database holding
    // such data must keep dropping them rather than creating empty tags.
    let source = Sandbox::new();
    source.command().arg("init").assert().success();
    common::seed_task(
        source.db_path(),
        "Edge case",
        "Critical",
        "Paused",
        Some("leading;;and;;double;;"),
    );

    let dest = Sandbox::new();
    dest.command().arg("init").assert().success();

    dest.command()
        .arg("import")
        .arg(source.db_path())
        .assert()
        .success();

    let imported = common::read_tasks(dest.db_path());
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].name, "Edge case");
    assert_eq!(
        imported[0].tags,
        vec![
            "and".to_string(),
            "double".to_string(),
            "leading".to_string()
        ]
    );
}

#[test]
fn import_rejects_missing_source_database() {
    let sb = Sandbox::new();
    sb.command().arg("init").assert().success();

    let missing = sb.path().join("does-not-exist.sqlite");
    // NOTE: Connection::open creates missing files, so a nonexistent source
    // doesn't fail at connect time — it fails when the task query runs
    // against the freshly created empty database.
    sb.command()
        .arg("import")
        .arg(&missing)
        .assert()
        .failure()
        .stderr(predicates::str::contains("no such table"));
}
