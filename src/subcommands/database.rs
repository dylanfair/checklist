use anyhow::{Context, Result};
use std::path::PathBuf;

use rusqlite::{params, Connection};

use crate::subcommands::config::{get_config_dir, read_config, Config};
use crate::subcommands::task::Task;

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
            status TEXT NOT NULL,
            date_added DATE NOT NULL,
            completed_on DATE
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
            status TEXT NOT NULL,
            date_added DATE NOT NULL,
            completed_on DATE
        )",
        (),
    )?;

    let config = Config::new(sqlite_path);
    config.save(testing)?;
    Ok(())
}

pub fn get_db(memory: bool, testing: bool) -> Result<Connection> {
    if memory {
        println!("Using an in-memory sqlite database");
        let conn = make_memory_connection().unwrap();
        Ok(conn)
    } else {
        let config = read_config(testing).context("Failed to read in config")?;
        let conn = make_connection(&config.db_path).with_context(|| {
            format!(
                "Failed to make a connection to the database: {:?}",
                config.db_path,
            )
        })?;
        Ok(conn)
    }
}

pub fn add_to_db(conn: &Connection, task: Task) -> Result<()> {
    println!("Adding to db");
    conn.execute(
        "INSERT INTO task (id, name, description, latest, urgency, status, date_added, completed_on) 
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        (
            &task.get_id(),
            &task.name,
            &task.description,
            &task.latest,
            &task.urgency,
            &task.status,
            &task.date_added,
            &task.completed_on,
        ),
    )
    .context("Failed to insert values into database")?;

    Ok(())
}

pub fn get_all_db_contents(conn: &Connection) -> Result<Vec<Task>> {
    let mut stmt = conn.prepare("SELECT * FROM task").unwrap();

    let task_iter = stmt
        .query_map(params![], |row| {
            Ok(Task::from_sql(
                row.get(0).unwrap(),
                row.get(1).unwrap(),
                row.get(2).unwrap(),
                row.get(3).unwrap(),
                row.get(4).unwrap(),
                row.get(5).unwrap(),
                row.get(6).unwrap(),
                row.get(7).unwrap(),
            ))
        })
        .unwrap();

    let mut task_vec = vec![];
    for task in task_iter {
        task_vec.push(task.unwrap());
    }

    Ok(task_vec)
}

pub fn remove_all_db_contents(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM task", ())
        .context("Failed to wipe the task table")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::subcommands::{config::read_config, task::Status, task::Urgency};
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

        let config = read_config(true).unwrap();
        assert_eq!(config.db_path.exists(), true);
        let _ = make_connection(&config.db_path).unwrap();

        wipe_existing_test_db(&test_db_path);
        assert_eq!(test_db_path.exists(), false);
    }

    #[test]
    fn add_to_database() {
        let conn = get_db(true, false).unwrap();

        let new_task = Task::new(
            "My new task".to_string(),
            None,
            None,
            Some(Urgency::Critical),
            Some(Status::Open),
        );
        add_to_db(&conn, new_task).unwrap();

        let task_vec = get_all_db_contents(&conn).unwrap();
        assert_eq!(task_vec.len(), 1);
    }
}
