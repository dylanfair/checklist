use anyhow::{Context, Result};
use std::path::PathBuf;

use rusqlite::Connection;

use crate::subcommands::config::{get_config_dir, read_config, Config};
use crate::subcommands::task::{Task, Urgency};

pub fn make_memory_connection() -> Result<Connection> {
    println!("Setting up an in-memory sqlite_db");
    let conn =
        Connection::open_in_memory().with_context(|| "Failed to create database in memory")?;

    conn.execute(
        "CREATE TABLE task (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            latest TEXT,
            urgency TEXT,
            status TEXT,
            completed_on TEXT
        )",
        (),
    )?;

    Ok(conn)
}

fn make_connection(path: &PathBuf) -> Result<Connection> {
    let conn = Connection::open(&path)
        .with_context(|| format!("Failed connect to the database at {:?}", path))?;

    Ok(conn)
}

pub fn create_sqlite_db(testing: bool) -> Result<()> {
    let local_config_dir = get_config_dir()?;
    let mut sqlite_path = local_config_dir;

    if testing {
        sqlite_path = sqlite_path.join("test.checklist.sqlite");
    } else {
        sqlite_path = sqlite_path.join("checklist.sqlite");
    }

    println!("Setting up a database at {:?}", sqlite_path);
    let conn = make_connection(&sqlite_path)?;

    conn.execute(
        "CREATE TABLE task (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            latest TEXT,
            urgency TEXT,
            status TEXT,
            completed_on TEXT
        )",
        (),
    )?;

    let config = Config::new(sqlite_path);
    config.save()?;
    Ok(())
}

fn get_db(memory: bool) -> Result<Connection> {
    if memory {
        println!("Using an in-memory sqlite database");
        let conn = make_memory_connection().unwrap();
        Ok(conn)
    } else {
        let config = read_config().context("Failed to read in config")?;
        let conn = make_connection(&config.db_path).with_context(|| {
            format!(
                "Failed to make a connection to the database: {:?}",
                config.db_path,
            )
        })?;
        Ok(conn)
    }
}

pub fn add_to_db(task: Task, memory: bool) -> Result<Connection> {
    println!("Connecting to db");
    let conn = get_db(memory)?;

    println!("Adding to db");
    conn.execute(
        "INSERT INTO task (id, name, description, latest, urgency, status, completed_on) 
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            &task.get_id(),
            &task.name,
            &task.description,
            &task.latest,
            &task.urgency,
            &task.status,
            &task.completed_on,
        ),
    )
    .context("Failed to insert values into database")?;

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use crate::subcommands::{config::read_config, task::Urgency};
    use std::fs::remove_file;

    use super::*;

    fn wipe_existing_test_db(test_db_path: &PathBuf) {
        if test_db_path.exists() {
            remove_file(test_db_path).unwrap();
        }
    }

    #[test]
    fn create_db() {
        let local_config_dir = get_config_dir().unwrap();
        let test_db_path = local_config_dir.join("test.checklist.sqlite");
        wipe_existing_test_db(&test_db_path);
        assert_eq!(test_db_path.exists(), false);

        create_sqlite_db(true).unwrap();

        let config = read_config().unwrap();
        assert_eq!(config.db_path.exists(), true);
        let _ = make_connection(&config.db_path).unwrap();

        wipe_existing_test_db(&test_db_path);
        assert_eq!(test_db_path.exists(), false);
    }

    #[test]
    fn add_to_database() {
        let new_task = Task::new(
            "My new task".to_string(),
            None,
            None,
            Some(Urgency::Critical),
        );
        let conn = add_to_db(new_task, true).unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, description, urgency, latest, status from task WHERE name = 'My new task'")
            .unwrap();

        let task_iter = stmt.query_map([], |row| {
            Ok(Task::from_sql(
                row.get(0).unwrap(),
                row.get(1).unwrap(),
                row.get(2).unwrap(),
                row.get(3).unwrap(),
                row.get(4).unwrap(),
                row.get(5).unwrap(),
                row.get(6).unwrap(),
            ))
        });
    }
}
