use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::backend::config::{Config, ConfigDir, read_config};
use crate::backend::task::{Task, TaskList};

/// Returns a `Result<Connection>` to an in-memory SQLite db
pub fn make_memory_connection() -> Result<Connection> {
    println!("Setting up an in-memory sqlite_db");
    let conn =
        Connection::open_in_memory().with_context(|| "Failed to create database in memory")?;
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

/// Returns a `Result<Connection>` given a `&Pathbuf` to a SQLite database
pub fn make_connection(path: &PathBuf) -> Result<Connection> {
    let conn = Connection::open(path)
        .with_context(|| format!("Failed connect to the database at {path:?}"))?;

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
/// otherwise ignored.
pub fn get_db(memory: bool, dir: &ConfigDir) -> Result<Connection> {
    if memory {
        println!("Using an in-memory sqlite database");
        let conn = make_memory_connection().unwrap();
        Ok(conn)
    } else {
        let config = read_config(dir).context("Failed to read in config")?;
        let conn = make_connection(&config.db_path).with_context(|| {
            format!(
                "Failed to make a connection to the database: {:?}",
                config.db_path,
            )
        })?;
        Ok(conn)
    }
}

/// Adds a `&Task` to a SQLite database based on the `&Connection` given.
pub fn add_to_db(conn: &Connection, task: &Task) -> Result<()> {
    // Handle inserting tags
    let mut tags_insert = None;
    if let Some(tags) = &task.tags {
        tags_insert = Some(tags.clone().into_iter().collect::<Vec<String>>().join(";"))
    }

    conn.execute(
        "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on) 
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            &task.get_id(),
            &task.name,
            &task.description,
            &task.latest,
            &task.urgency,
            &task.status,
            tags_insert,
            &task.get_date_added(),
            &task.completed_on,
        ],
    )
    .context("Failed to insert values into database")?;

    Ok(())
}

/// Updates a `&Task` in a SQLite database based on the `&Connecton` given.
pub fn update_task_in_db(conn: &Connection, task: &Task) -> Result<()> {
    let mut tags_insert = None;
    if let Some(tags) = &task.tags {
        tags_insert = Some(tags.clone().into_iter().collect::<Vec<String>>().join(";"))
    }

    conn.execute(
        "UPDATE task SET name = ?1, description = ?2, latest = ?3, urgency = ?4, status = ?5, tags = ?6, date_added = ?7, completed_on = ?8 WHERE id = ?9"
        ,params![
            &task.name,
            &task.description,
            &task.latest,
            &task.urgency,
            &task.status,
            tags_insert,
            &task.get_date_added(),
            &task.completed_on,
            &task.get_id()
        ]
            ).context("Failed to update values for the task")?;

    Ok(())
}

/// Deletes a `&Task` in a SQLite database based on the `&Connecton` given.
pub fn delete_task_in_db(conn: &Connection, task: &Task) -> Result<()> {
    // println!("Deleting task from db");
    conn.execute("DELETE FROM task WHERE id = ?1", params![&task.get_id()])
        .context("Failed to delete task from the database")?;
    Ok(())
}

/// Returns a `Result<TaskList>` of all tasks in a SQLite database on the `&Connection` given.
///
/// Rows that fail to deserialize (e.g. a NULL in a NOT NULL column, or a
/// structurally malformed row) are skipped with a warning printed to stderr
/// rather than aborting the entire load. A bad `urgency`/`status` string is
/// handled separately: `FromSql` falls back to the default variant, so those
/// rows still load (see `task::Urgency` / `task::Status`).
pub fn get_all_db_contents(conn: &Connection) -> Result<TaskList> {
    let mut stmt = conn
        .prepare("SELECT * FROM task")
        .context("Failed to prepare the 'task' query")?;

    let task_iter = stmt
        .query_map(params![], |row| {
            // Need separate handling for the tags
            // Basically convert string back to a vector
            let mut tags_entry = None;
            let tags_option: Option<String> = row.get(6)?;

            if let Some(tags) = tags_option {
                let tags_vec: Vec<String> = tags
                    .split(';')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect();
                tags_entry = Some(HashSet::from_iter(tags_vec));
            }

            Ok(Task::from_sql(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                tags_entry,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .context("Failed to map over the 'task' rows")?;

    let mut task_list = TaskList::new();
    let mut skipped = 0usize;
    for task in task_iter {
        match task {
            Ok(t) => task_list.tasks.push(t),
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
