// Shared mechanics for checklist's black-box CLI tests.
//
// This module is compiled separately into every test crate that declares
// `mod common;`, so not every helper is used by every consumer — hence the
// blanket allow rather than chasing per-use dead_code warnings.
#![allow(dead_code)]

use assert_cmd::Command;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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

/// A task read back from a database file, normalized across schema formats
/// (legacy `;-joined tags column vs relational `tag` table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub name: String,
    pub urgency: String,
    pub status: String,
    /// Sorted so comparisons don't depend on storage order.
    pub tags: Vec<String>,
}

/// Insert one task the way the ORIGINAL writer did: a raw insert into the
/// legacy layout with an optionally ;-joined tags string. Works on any
/// database that still has the old-style `task` table — which includes both
/// freshly initialized databases and genuine pre-V1 ones.
pub fn seed_task(
    db_path: impl AsRef<Path>,
    name: &str,
    urgency: &str,
    status: &str,
    tags: Option<&str>,
) {
    let conn = rusqlite::Connection::open(db_path).expect("failed to open db for seeding");
    conn.execute(
        "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on)
         VALUES (?1, ?2, NULL, NULL, ?3, ?4, ?5, datetime('now'), NULL)",
        rusqlite::params![uuid::Uuid::new_v4(), name, urgency, status, tags],
    )
    .expect("failed to seed task");
}

/// Count rows in the `task` table.
pub fn task_count(db_path: impl AsRef<Path>) -> i64 {
    let conn = rusqlite::Connection::open(db_path).expect("failed to open db");
    conn.query_row("SELECT COUNT(*) FROM task", [], |row| row.get(0))
        .expect("failed to count tasks")
}

/// Read every task back as summaries. Handles both the legacy format (tags
/// in a ;-joined TEXT column) and the current one (tags in the `tag` table),
/// mirroring what the app itself does when reading.
pub fn read_tasks(db_path: impl AsRef<Path>) -> Vec<TaskSummary> {
    let conn = rusqlite::Connection::open(db_path).expect("failed to open db");

    let has_legacy_tags_column = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(task)")
            .expect("failed to inspect schema");
        let names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("failed to read schema")
            .collect::<std::result::Result<_, _>>()
            .expect("failed to read schema");
        names.iter().any(|n| n == "tags")
    };

    let sql = if has_legacy_tags_column {
        "SELECT id, name, urgency, status, tags FROM task"
    } else {
        "SELECT id, name, urgency, status, NULL FROM task"
    };
    let mut stmt = conn.prepare(sql).expect("failed to prepare task query");

    let mut tasks: Vec<TaskSummary> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, uuid::Uuid>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .expect("failed to query tasks")
        .map(|row| {
            let (id, name, urgency, status, tags_raw) = row.expect("failed to read task row");

            let mut tags: Vec<String> = if !has_legacy_tags_column {
                conn.prepare("SELECT tag FROM tag WHERE task_id = ?1")
                    .expect("failed to prepare tag query")
                    .query_map([&id], |row| row.get::<_, String>(0))
                    .expect("failed to query tags")
                    .collect::<std::result::Result<_, _>>()
                    .expect("failed to collect tags")
            } else {
                tags_raw
                    .unwrap_or_default()
                    .split(';')
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            tags.sort();

            TaskSummary {
                name,
                urgency,
                status,
                tags,
            }
        })
        .collect();

    tasks.sort_by(|a, b| a.name.cmp(&b.name));
    tasks
}
