use anyhow::{Context, Result};
use std::path::PathBuf;

use rusqlite::Connection;

fn make_connection(path: Option<PathBuf>, memory_db: bool) -> Result<Connection> {
    if memory_db {
        println!("Setting up an in-memory sqlite_db");
        let conn =
            Connection::open_in_memory().with_context(|| "Failed to create database in memory")?;

        Ok(conn)
    } else {
        let mut sqlite_path = path.unwrap_or(PathBuf::from("."));
        sqlite_path.push("checklist.sqlite");
        println!("Setting up a database at {:?}", sqlite_path);
        let conn = Connection::open(&sqlite_path)
            .with_context(|| format!("Failed to create the database at {:?}", sqlite_path))?;

        Ok(conn)
    }
}

pub fn create_sqlite_db(path: Option<PathBuf>, memory_db: bool) -> Result<()> {
    let conn = make_connection(path, memory_db)?;

    Ok(())
}
