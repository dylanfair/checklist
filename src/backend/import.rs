use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

use crate::backend::database::{add_to_db, get_all_db_contents, make_connection};

pub fn import_database(database_path: PathBuf, dest_conn: &Connection) -> Result<()> {
    // read in tasks from database to be imported
    // then add them to current database

    let new_db_conn = make_connection(&database_path)?;

    let new_db_tasks = get_all_db_contents(&new_db_conn)?;
    println!("Adding {} tasks to current database", new_db_tasks.len());
    let mut failed_tasks = vec![];
    for task in new_db_tasks.tasks {
        match add_to_db(dest_conn, &task) {
            Ok(_) => {}
            Err(_) => {
                failed_tasks.push(task);
            }
        }
    }

    if !failed_tasks.is_empty() {
        eprintln!("{} tasks failed to get moved over.", failed_tasks.len());
        eprintln!("Failed task ids:");
        for task in failed_tasks {
            eprintln!("{}", task.get_id());
        }
    }
    Ok(())
}
