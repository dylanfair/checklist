use anyhow::{Context, Result};
use std::path::PathBuf;

use rusqlite::Connection;

use crate::subcommands::config::{get_config_dir, Config};

fn make_memory_connection() -> Result<Connection> {
    println!("Setting up an in-memory sqlite_db");
    let conn =
        Connection::open_in_memory().with_context(|| "Failed to create database in memory")?;

    Ok(conn)
}

fn make_connection(path: &PathBuf) -> Result<Connection> {
    let conn = Connection::open(&path)
        .with_context(|| format!("Failed to create the database at {:?}", path))?;

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
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            urgency TEXT
        )",
        (),
    )?;

    let config = Config::new(sqlite_path);
    config.save()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::subcommands::config::read_config;
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
}
