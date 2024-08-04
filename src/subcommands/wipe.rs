use crate::subcommands::database::remove_all_db_contents;
use anyhow::Result;
use rusqlite::Connection;

pub fn wipe_tasks(conn: &Connection, confirm_skip: bool) -> Result<()> {
    if !confirm_skip {
        println!("Are you sure you want to wipe out all your tasks? (y/n)");
        loop {
            let mut confirmation = String::new();
            std::io::stdin().read_line(&mut confirmation).unwrap();

            let confirmation = confirmation.replace("\n", "").to_lowercase();
            let confirmation_str = confirmation.as_str();
            println!("{}", confirmation_str);
            if confirmation_str == "y" || confirmation_str == "n" {
                println!("You must provide either a 'y' or 'n'");
                continue;
            }

            if confirmation_str == "y" {
                break;
            }
            println!("Not wiping all tasks");
            return Ok(());
        }
    }
    remove_all_db_contents(&conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subcommands::database::{add_to_db, get_all_db_contents, get_db};
    use crate::subcommands::task::{Task, Urgency};

    #[test]
    fn test_wipe_tasks() {
        let conn = get_db(true).unwrap();

        let new_task = Task::new(
            String::from("Task1"),
            Some(String::from("A description")),
            Some(String::from("A latest")),
            None,
        );
        let second_new_task = Task::new(
            String::from("Task2"),
            Some(String::from("Another description")),
            Some(String::from("A latest")),
            Some(Urgency::Medium),
        );

        add_to_db(&conn, new_task).unwrap();
        add_to_db(&conn, second_new_task).unwrap();

        let tasks = get_all_db_contents(&conn).unwrap();
        assert_eq!(tasks.len(), 2);

        remove_all_db_contents(&conn).unwrap();
        let tasks = get_all_db_contents(&conn).unwrap();
        assert_eq!(tasks.len(), 0);
    }
}
