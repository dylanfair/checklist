use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::backend::config::{Config, ConfigDir, read_config};
use crate::backend::task::{Task, TaskList};

/// Returns a `Result<Connection>` to an in-memory SQLite db
pub fn make_memory_connection() -> Result<Connection> {
    println!("Setting up an in-memory sqlite_db");
    let mut conn =
        Connection::open_in_memory().with_context(|| "Failed to create database in memory")?;
    enable_foreign_keys(&mut conn);
    init_schema(&conn)?;
    Ok(conn)
}

/// Creates the `task` table on an open connection. Shared between the
/// in-memory connection, the default-path bootstrap, and bootstrap at an
/// arbitrary path so the schema string lives in one place.
fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE task (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            latest TEXT,
            urgency TEXT,
            status TEXT NOT NULL,
            tags TEXT,
            date_added DATE NOT NULL,
            completed_on DATE
        )",
        (),
    )?;
    Ok(())
}

/// Enable foreign key enforcement on a connection. SQLite ships with it OFF
/// per-connection; without this, the tag table's ON DELETE CASCADE would not
/// fire. (Note: rusqlite_migration flips this off/on around migrations; see
/// `migrate::run_migrations`.)
fn enable_foreign_keys(conn: &mut Connection) {
    if let Err(e) = conn.pragma_update(None, "foreign_keys", "ON") {
        eprintln!("checklist: could not enable foreign key enforcement: {e}");
    }
}

/// Returns a `Result<Connection>` given a `&Pathbuf` to a SQLite database
pub fn make_connection(path: &PathBuf) -> Result<Connection> {
    let mut conn = Connection::open(path)
        .with_context(|| format!("Failed connect to the database at {path:?}"))?;
    enable_foreign_keys(&mut conn);

    Ok(conn)
}

/// Bootstrap a new SQLite database (with the `task` table) at `db_path`.
/// Opens a connection, creates the schema, and drops the connection. Does not
/// touch `config.json` — the caller is responsible for pointing the config at
/// `db_path` (e.g. via `set_new_path`).
pub fn create_sqlite_db_at(db_path: PathBuf) -> Result<()> {
    println!("Setting up a database at {db_path:?}");
    let conn = make_connection(&db_path)?;
    init_schema(&conn)?;
    // `conn` is dropped here, closing the database.
    Ok(())
}

/// Creates a SQLite database in `dir` (at the default `checklist.sqlite`
/// path) and records that path in a new `config.json` there.
///
/// Used by `checklist init` (without `--set`) to bootstrap at the default
/// location. For bootstrap at an arbitrary path, use `create_sqlite_db_at`
/// and then write the config separately.
pub fn create_sqlite_db(dir: &ConfigDir) -> Result<()> {
    let sqlite_path = dir.db_path();
    create_sqlite_db_at(sqlite_path.clone())?;
    let config = Config::new(sqlite_path);
    config.save(dir)?;
    Ok(())
}

/// Returns a `Result<Connection>` based on `memory` and the config directory.
///
/// When `memory` is true, an in-memory SQLite database is used and `dir` is
/// otherwise ignored. In both cases, any pending schema migrations are applied
/// before the connection is returned (a no-op on an already-current database).
pub fn get_db(memory: bool, dir: &ConfigDir) -> Result<Connection> {
    if memory {
        println!("Using an in-memory sqlite database");
        let mut conn = make_memory_connection()?;
        crate::backend::migrate::run_migrations(&mut conn, None)?;
        Ok(conn)
    } else {
        let config = if dir.config_path().exists() {
            read_config(dir).context(format!(
                "Failed to read in config in location: {:?}",
                dir.config_path()
            ))?
        } else {
            let new_config = Config::new(dir.db_path());
            new_config.save(dir).context(format!(
                "Failed to save config in location: {:?}",
                dir.config_path()
            ))?;
            new_config
        };
        let mut conn = make_connection(&config.db_path).with_context(|| {
            format!(
                "Failed to make a connection to the database: {:?}",
                config.db_path,
            )
        })?;
        // A brand-new database file has no tables yet; create the baseline
        // schema so migrations have something to bring forward. A freshly
        // created file holds no user data, so no pre-migration backup is
        // needed for it.
        let freshly_created = ensure_baseline_schema(&conn)?;
        if freshly_created {
            crate::backend::migrate::run_migrations(&mut conn, None)?;
        } else {
            crate::backend::migrate::run_migrations(&mut conn, Some(&config.db_path))?;
        }
        Ok(conn)
    }
}

/// If the `task` table doesn't exist at all (a brand-new database file),
/// create the baseline schema. Migrations then bring the database current,
/// so every checklist-managed database follows the same version-0-to-latest
/// path regardless of how it came into existence.
///
/// Returns whether the baseline was just created (i.e. the file held no data
/// worth backing up).
fn ensure_baseline_schema(conn: &Connection) -> Result<bool> {
    // PRAGMA table_info returns one row per column; zero rows means the
    // table is absent.
    let mut stmt = conn
        .prepare("PRAGMA table_info(task)")
        .context("Failed to inspect the 'task' table schema")?;
    let columns = stmt
        .query_map(params![], |row| row.get::<_, String>(1))
        .context("Failed to read the 'task' table schema")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to read the 'task' table schema")?;
    let missing = columns.is_empty();
    if missing {
        init_schema(conn)?;
    }
    Ok(missing)
}

/// Writes a task's tags into the `tag` table, replacing any existing rows for
/// that task. An empty/`None` set clears the task's tags.
fn write_task_tags(
    conn: &Connection,
    task_id: &uuid::Uuid,
    tags: &Option<HashSet<String>>,
) -> Result<()> {
    conn.execute("DELETE FROM tag WHERE task_id = ?1", params![task_id])
        .context("Failed to clear existing tags for the task")?;
    if let Some(tags) = tags {
        for tag in tags {
            conn.execute(
                "INSERT INTO tag (task_id, tag) VALUES (?1, ?2)",
                params![task_id, tag],
            )
            .context("Failed to insert a tag for the task")?;
        }
    }
    Ok(())
}

/// Reads a task's tags from the `tag` table. Returns `None` when the task has
/// no tags.
fn get_task_tags(conn: &Connection, task_id: &uuid::Uuid) -> Result<Option<HashSet<String>>> {
    let mut stmt = conn
        .prepare("SELECT tag FROM tag WHERE task_id = ?1")
        .context("Failed to prepare the tag query for a task")?;
    let tags = stmt
        .query_map(params![task_id], |row| row.get::<_, String>(0))
        .context("Failed to read tags for the task")?
        .collect::<std::result::Result<HashSet<_>, _>>()
        .context("Failed to collect tags for the task")?;
    Ok(if tags.is_empty() { None } else { Some(tags) })
}

/// Adds a `&Task` to a SQLite database based on the `&Connection` given.
pub fn add_to_db(conn: &Connection, task: &Task) -> Result<()> {
    conn.execute(
        "INSERT INTO task (id, name, description, latest, urgency, status, date_added, completed_on) 
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &task.get_id(),
            &task.name,
            &task.description,
            &task.latest,
            &task.urgency,
            &task.status,
            &task.get_date_added(),
            &task.completed_on,
        ],
    )
    .context("Failed to insert values into database")?;

    write_task_tags(conn, &task.get_id(), &task.tags)?;

    Ok(())
}

/// Updates a `&Task` in a SQLite database based on the `&Connecton` given.
pub fn update_task_in_db(conn: &Connection, task: &Task) -> Result<()> {
    conn.execute(
        "UPDATE task SET name = ?1, description = ?2, latest = ?3, urgency = ?4, status = ?5, date_added = ?6, completed_on = ?7 WHERE id = ?8"
        ,params![
            &task.name,
            &task.description,
            &task.latest,
            &task.urgency,
            &task.status,
            &task.get_date_added(),
            &task.completed_on,
            &task.get_id()
        ]
            ).context("Failed to update values for the task")?;

    // Tags are stored relationally: replace the old set wholesale.
    write_task_tags(conn, &task.get_id(), &task.tags)?;

    Ok(())
}

/// Deletes a `&Task` in a SQLite database based on the `&Connecton` given.
pub fn delete_task_in_db(conn: &Connection, task: &Task) -> Result<()> {
    // The tag table's FK declares ON DELETE CASCADE, so its rows go with the
    // task (connections enable PRAGMA foreign_keys).
    conn.execute("DELETE FROM task WHERE id = ?1", params![&task.get_id()])
        .context("Failed to delete task from the database")?;
    Ok(())
}

/// Detect whether the `task` table carries the legacy ;-joined `tags` column
/// (pre-V1 format) or not (V1+ format). Used because databases we *read* may
/// be older than us — notably import sources — while ones we *write* are
/// always migrated current first.
fn has_legacy_tags_column(conn: &Connection) -> Result<bool> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(task)")
        .context("Failed to inspect the 'task' table schema")?;
    let names = stmt
        .query_map(params![], |row| row.get::<_, String>(1))
        .context("Failed to read the 'task' table schema")?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to read the 'task' table schema")?;
    Ok(names.iter().any(|n| n == "tags"))
}

/// Returns a `Result<TaskList>` of all tasks in a SQLite database on the `&Connection` given.
///
/// Reads both the current relational format and the legacy ;-joined `tags`
/// column, so older databases (e.g. an import source) load losslessly.
///
/// Rows that fail to deserialize (e.g. a NULL in a NOT NULL column, or a
/// structurally malformed row) are skipped with a warning printed to stderr
/// rather than aborting the entire load. A bad `urgency`/`status` string is
/// handled separately: `FromSql` falls back to the default variant, so those
/// rows still load (see `task::Urgency` / `task::Status`).
pub fn get_all_db_contents(conn: &Connection) -> Result<TaskList> {
    let legacy_tags = has_legacy_tags_column(conn)?;

    // Explicit column list rather than SELECT * so positional reads can't
    // silently shift if the schema changes again.
    let sql = if legacy_tags {
        "SELECT id, name, description, latest, urgency, status, tags, date_added, completed_on
         FROM task"
    } else {
        "SELECT id, name, description, latest, urgency, status, date_added, completed_on
         FROM task"
    };
    let mut stmt = conn
        .prepare(sql)
        .context("Failed to prepare the 'task' query")?;

    let task_iter = stmt
        .query_map(params![], move |row| {
            let tags_entry = if legacy_tags {
                // Legacy format: split the ;-joined string, dropping empty
                // segments exactly as the pre-migration reader did.
                let tags_option: Option<String> = row.get(6)?;
                tags_option.map(|tags| {
                    HashSet::from_iter(
                        tags.split(';')
                            .filter(|p| !p.is_empty())
                            .map(str::to_string),
                    )
                })
            } else {
                None // relational tags filled in per-task below
            };

            let (date_added_idx, completed_idx) = if legacy_tags { (7, 8) } else { (6, 7) };

            Ok(Task::from_sql(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                tags_entry,
                row.get(date_added_idx)?,
                row.get(completed_idx)?,
            ))
        })
        .context("Failed to map over the 'task' rows")?;

    let mut task_list = TaskList::new();
    let mut skipped = 0usize;
    for task in task_iter {
        match task {
            Ok(mut t) => {
                if !legacy_tags {
                    t.tags = get_task_tags(conn, &t.get_id())?;
                }
                task_list.tasks.push(t);
            }
            Err(e) => {
                skipped += 1;
                eprintln!("checklist: skipping a malformed task row: {e}");
            }
        }
    }
    if skipped > 0 {
        eprintln!("checklist: {skipped} task(s) skipped due to load errors");
    }

    Ok(task_list)
}

/// Deletes all tasks in a SQLite database on the `&Connection` given.
/// If `hard` is true, this will also DROP the task table.
pub fn remove_all_db_contents(conn: &Connection, hard: bool) -> Result<()> {
    if hard {
        conn.execute("DROP TABLE task", ())
            .context("Failed to drop the task table")?;
        println!("'task' table dropped successfully");
    } else {
        conn.execute("DELETE FROM task", ())
            .context("Failed to wipe all tasks from the task table")?;
        println!("Tasks from 'task' table deleted successfully");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::backend::config::{ConfigDir, read_config};
    use crate::backend::task::{Status, Urgency};
    use std::collections::HashSet;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn get_db_self_heals_a_brand_new_install() {
        // The true first-run state: no config.json, no database file, nothing.
        // get_db should create both and hand back a migrated connection that
        // can immediately store tasks — without any prior `checklist init`.
        let tmp = tempdir().unwrap();
        let dir = ConfigDir::new(tmp.path().to_path_buf());
        assert!(!dir.config_path().exists());
        assert!(!dir.db_path().exists());

        let conn = get_db(false, &dir).unwrap();

        assert!(dir.config_path().exists(), "config should have been created");
        let config = read_config(&dir).unwrap();
        assert!(config.db_path.exists(), "database file should have been created");

        // And it should actually be usable end-to-end.
        use crate::backend::task::{Status, Task, Urgency};
        let task = Task::new(
            "First task".to_string(),
            None,
            None,
            Some(Urgency::Low),
            Some(Status::Open),
            Some(HashSet::from_iter(["setup".to_string()])),
        );
        add_to_db(&conn, &task).unwrap();
        let loaded = get_all_db_contents(&conn).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.tasks[0].name, "First task");
        assert_eq!(
            loaded.tasks[0].tags,
            Some(HashSet::from_iter(["setup".to_string()]))
        );
    }

    #[test]
    fn create_db() {
        let tmp = tempdir().unwrap();
        let dir = ConfigDir::new(tmp.path().to_path_buf());
        let db_path = dir.db_path();
        assert!(!db_path.exists());

        create_sqlite_db(&dir).unwrap();

        let config = read_config(&dir).unwrap();
        assert!(config.db_path.exists());
        let _ = make_connection(&config.db_path).unwrap();
        // tempdir is removed automatically on drop; no manual wipe needed.
    }

    #[test]
    fn add_delete_to_database() {
        let tmp = tempdir().unwrap();
        let dir = ConfigDir::new(tmp.path().to_path_buf());
        let conn = get_db(true, &dir).unwrap();

        let new_task = Task::new(
            "My new task".to_string(),
            Some("New description".to_string()),
            Some("New latest".to_string()),
            Some(Urgency::Critical),
            Some(Status::Open),
            Some(HashSet::from_iter(vec![
                String::from("Tag1"),
                String::from("Tag2"),
            ])),
        );
        add_to_db(&conn, &new_task).unwrap();

        // Check if data we get back from database matches
        let task_list = get_all_db_contents(&conn).unwrap();
        assert_eq!(task_list.len(), 1);
        let task = task_list.tasks.first().unwrap();
        assert_eq!(task.name, "My new task".to_string());
        assert_eq!(task.description, Some("New description".to_string()));
        assert_eq!(task.latest, Some("New latest".to_string()));
        assert_eq!(task.urgency, Urgency::Critical);
        assert_eq!(task.status, Status::Open);
        assert_eq!(
            task.tags,
            Some(HashSet::from_iter(vec![
                String::from("Tag1"),
                String::from("Tag2"),
            ]))
        );
        assert!(task.completed_on.is_none());

        // Let's see if delete works as well!
        delete_task_in_db(&conn, &new_task).unwrap();
        let task_list = get_all_db_contents(&conn).unwrap();
        assert_eq!(task_list.len(), 0);
    }
}
