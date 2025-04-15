use anyhow::Result;
use std::path::PathBuf;

use crate::backend::database::{add_to_db, get_all_db_contents, make_connection};

use super::config::Config;

pub fn import_database(database_path: String, config: Config) -> Result<()> {
    // read in tasks from database to be imported
    // then add them to current database

    let new_db = PathBuf::from(database_path);
    let new_db_conn = make_connection(&new_db)?;
    let existing_db = config.db_path;
    let existing_db_conn = make_connection(&existing_db)?;

    let new_db_tasks = get_all_db_contents(&new_db_conn)?;
    println!("Adding {} tasks to current database", new_db_tasks.len());
    let mut failed_tasks = vec![];
    for task in new_db_tasks.tasks {
        match add_to_db(&existing_db_conn, &task) {
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
