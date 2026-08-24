use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

use crate::backend::{
    config::{ConfigDir, read_config},
    database::{
        add_to_db, create_sqlite_db, get_all_db_contents, make_connection, make_memory_connection,
    },
    migrate::run_migrations,
};

pub fn import(database: PathBuf, memory: bool, config_directory: &ConfigDir) -> Result<Connection> {
    let (mut dest_conn, dest_path) = if memory {
        (make_memory_connection()?, None)
    } else {
        let config = match read_config(config_directory) {
            Ok(config) => config,
            Err(_) => {
                create_sqlite_db(config_directory)?;
                println!("Could not find an existing database, creating a new one.");
                read_config(config_directory)?
            }
        };
        let conn = make_connection(&config.db_path)?;
        (conn, Some(config.db_path))
    };

    // The destination may predate the migration system (version 0); bring it
    // current before writing, since add_to_db writes tags relationally. Back
    // up the destination file first — it can hold real user data.
    run_migrations(&mut dest_conn, dest_path.as_deref())?;

    import_database(database, &dest_conn)?;
    println!("Finished importing tasks to current database.");
    Ok(dest_conn)
}

fn import_database(database_path: PathBuf, dest_conn: &Connection) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use crate::backend::config::ConfigDir;
    use crate::backend::task::{Status, Urgency};
    use std::collections::HashSet;
    use tempfile::tempdir;

    use super::*;

    /// Seed a task into a database the way the OLD writer did: raw insert with
    /// a ;-joined tags string. Used to simulate pre-V1 databases.
    fn seed_task_old_style(conn: &Connection) {
        conn.execute(
            "INSERT INTO task (id, name, description, latest, urgency, status, tags, date_added, completed_on)
             VALUES (?1, 'My new task', 'New description', 'New latest', 'Critical', 'Open', 'Tag1;Tag2', ?2, NULL)",
            rusqlite::params![uuid::Uuid::new_v4(), chrono::Local::now()],
        )
        .unwrap();
    }

    #[test]
    fn import_memory() {
        let tmp = tempdir().unwrap();
        let dir = ConfigDir::new(tmp.path().to_path_buf());
        let db_path = dir.db_path();
        assert!(!db_path.exists());

        // Make a temp disk db and add a task to it. The source stays at the
        // OLD format (version 0, ;-joined tags) to prove imports from
        // pre-migration databases are lossless.
        create_sqlite_db(&dir).unwrap();
        let disk_conn = make_connection(&db_path).unwrap();
        seed_task_old_style(&disk_conn);

        // Import from temp disk db to a memory db
        let mut memory_conn = make_memory_connection().unwrap();
        run_migrations(&mut memory_conn, None).unwrap();
        import_database(db_path, &memory_conn).unwrap();

        // Check if task is in memory db
        let task_list = get_all_db_contents(&memory_conn).unwrap();
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
    }

    #[test]
    fn import_disk() {
        let tmp = tempdir().unwrap();
        let dir = ConfigDir::new(tmp.path().to_path_buf());
        let db_path = dir.db_path();
        assert!(!db_path.exists());

        let tmp2 = tempdir().unwrap();
        let dir2 = ConfigDir::new(tmp2.path().to_path_buf());
        let db_path2 = dir2.db_path();
        assert!(!db_path2.exists());

        // Make a temp disk db (old-format source) and add a task to it
        create_sqlite_db(&dir).unwrap();
        create_sqlite_db(&dir2).unwrap();

        let disk_conn1 = make_connection(&db_path).unwrap();
        seed_task_old_style(&disk_conn1);

        // Destination is migrated current first, as import() does.
        let mut disk_conn2 = make_connection(&db_path2).unwrap();
        run_migrations(&mut disk_conn2, None).unwrap();

        // Import from temp disk db to another disk db
        import_database(db_path, &disk_conn2).unwrap();

        // Check if task is in the other disk db
        let task_list = get_all_db_contents(&disk_conn2).unwrap();
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
    }
}
