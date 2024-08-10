use anyhow::Result;
use rusqlite::Connection;

use crate::backend::database::remove_all_db_contents;

pub fn wipe_tasks(conn: &Connection, confirm_skip: bool, hard: bool) -> Result<()> {
    if !confirm_skip {
        println!("Are you sure you want to proceed with the wipe? (y/n)");
        loop {
            let mut confirmation = String::new();
            std::io::stdin().read_line(&mut confirmation).unwrap();

            match confirmation.to_lowercase().trim_end() {
                "y" => break,
                "n" => {
                    println!("Halting wipe");
                    return Ok(());
                }
                _ => println!("You must provide either a 'y' or 'n'"),
            }
        }
    }
    println!("Proceeding with wipe");
    remove_all_db_contents(&conn, hard)?;
    println!("Success!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::database::{add_to_db, get_all_db_contents, get_db};
    use crate::backend::task::{Status, Task, Urgency};
    use std::collections::HashSet;

    #[test]
    fn test_wipe_tasks() {
        let conn = get_db(true, false).unwrap();

        let new_task = Task::new(
            String::from("Task1"),
            Some(String::from("A description")),
            Some(String::from("A latest")),
            None,
            Some(Status::Open),
            Some(HashSet::from_iter(vec![
                String::from("Tag3"),
                String::from("Tag4"),
            ])),
        );
        let second_new_task = Task::new(
            String::from("Task2"),
            Some(String::from("Another description")),
            Some(String::from("A latest")),
            Some(Urgency::Medium),
            Some(Status::Paused),
            Some(HashSet::from_iter(vec![String::from("Tag1")])),
        );

        add_to_db(&conn, &new_task).unwrap();
        add_to_db(&conn, &second_new_task).unwrap();

        let task_list = get_all_db_contents(&conn).unwrap();
        assert_eq!(task_list.len(), 2);

        remove_all_db_contents(&conn, false).unwrap();
        let task_list = get_all_db_contents(&conn).unwrap();
        assert_eq!(task_list.len(), 0);
    }
}
