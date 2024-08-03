use anyhow::{Context, Result};
use std::path::PathBuf;

use rusqlite::Connection;

use crate::subcommands::config;

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

pub fn create_sqlite_db(path: Option<PathBuf>, memory_db: bool) -> Result<()> {
    if memory_db {
        let _ = make_memory_connection();
        Ok(())
    } else {
        let mut sqlite_path = path.unwrap_or(PathBuf::from("."));
        sqlite_path.push("checklist.sqlite");
        println!("Setting up a database at {:?}", sqlite_path);

        //let _ = make_connection(&sqlite_path)?;
        config::save_db_path(sqlite_path)?;
        Ok(())
    }
}
