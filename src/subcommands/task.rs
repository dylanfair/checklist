use chrono::prelude::*;
use clap::ValueEnum;
use crossterm::style::Stylize;
use rusqlite::{types::FromSql, types::ValueRef, ToSql};
use std::cmp::Ordering;
use std::string::ToString;
use uuid::Uuid;

#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum, strum_macros::Display)]
pub enum Urgency {
    Low,
    Medium,
    High,
    Critical,
}

impl Urgency {
    fn to_colored_string(&self) -> crossterm::style::StyledContent<String> {
        match self {
            Urgency::Low => String::from("Low").green(),
            Urgency::Medium => String::from("Medium").yellow(),
            Urgency::High => String::from("High").dark_yellow(),
            Urgency::Critical => String::from("Critical").red(),
        }
    }
}

impl From<&str> for Urgency {
    fn from(s: &str) -> Self {
        match s {
            "Low" => Urgency::Low,
            "Medium" => Urgency::Medium,
            "High" => Urgency::High,
            "Critical" => Urgency::Critical,
            _ => {
                println!("String received was not a valid Urgency");
                panic!()
            }
        }
    }
}

impl ToSql for Urgency {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for Urgency {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(Into::into)
    }
}

#[derive(Clone, Debug, Copy, ValueEnum, strum_macros::Display)]
pub enum Status {
    Open,
    Working,
    Paused,
    Completed,
}

impl Status {
    fn to_colored_string(&self) -> crossterm::style::StyledContent<String> {
        match self {
            Status::Open => String::from("Open").cyan(),
            Status::Working => String::from("Working").dark_green(),
            Status::Paused => String::from("Paused").dark_yellow(),
            Status::Completed => String::from("Completed").green(),
        }
    }
}

impl From<&str> for Status {
    fn from(s: &str) -> Self {
        match s {
            "Open" => Status::Open,
            "Working" => Status::Working,
            "Paused" => Status::Paused,
            "Completed" => Status::Completed,
            _ => {
                println!("String received wasn not a valid Status");
                panic!()
            }
        }
    }
}

impl ToSql for Status {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for Status {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        value.as_str().map(Into::into)
    }
}

#[derive(Clone, Debug)]
pub struct Task {
    id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub latest: Option<String>,
    pub urgency: Urgency,
    pub status: Status,
    pub tags: Option<Vec<String>>,
    date_added: DateTime<Local>,
    pub completed_on: Option<DateTime<Local>>,
}

impl Task {
    pub fn new(
        name: String,
        description: Option<String>,
        latest: Option<String>,
        urgency: Option<Urgency>,
        status: Option<Status>,
        tags: Option<Vec<String>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            latest,
            urgency: urgency.unwrap_or(Urgency::Low),
            status: status.unwrap_or(Status::Open),
            tags,
            date_added: Local::now(),
            completed_on: None,
        }
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn get_date_added(&self) -> DateTime<Local> {
        self.date_added
    }

    pub fn from_sql(
        id: Uuid,
        name: String,
        description: Option<String>,
        latest: Option<String>,
        urgency: Urgency,
        status: Status,
        tags: Option<Vec<String>>,
        date_added: DateTime<Local>,
        completed_on: Option<DateTime<Local>>,
    ) -> Self {
        Self {
            id,
            name,
            description,
            latest,
            urgency,
            status,
            tags,
            date_added,
            completed_on,
        }
    }
}

fn urgency_desc(a: &Task, b: &Task) -> Ordering {
    if a.urgency < b.urgency {
        return Ordering::Greater;
    }
    if a.urgency == b.urgency {
        if a.date_added > b.date_added {
            return Ordering::Less;
        }
        return Ordering::Greater;
    }
    Ordering::Less
}

fn urgency_asc(a: &Task, b: &Task) -> Ordering {
    if a.urgency > b.urgency {
        return Ordering::Greater;
    }
    if a.urgency == b.urgency {
        if a.date_added > b.date_added {
            return Ordering::Greater;
        }
        return Ordering::Less;
    }
    Ordering::Less
}

#[derive(Clone, Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
}

impl TaskList {
    pub fn new() -> Self {
        TaskList { tasks: vec![] }
    }

    pub fn from(tasks: Vec<Task>) -> Self {
        TaskList { tasks }
    }

    pub fn sort_by_urgency(&mut self, descending: bool) {
        if descending {
            self.tasks.sort_by(urgency_desc)
        } else {
            self.tasks.sort_by(urgency_asc)
        }
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn display_tasks(&self) {
        for (i, task) in self.tasks.iter().enumerate() {
            let name = task.name.clone();
            let description = task.description.clone().unwrap_or(String::from("None"));
            let latest = task.latest.clone().unwrap_or(String::from("None"));
            //let tags = task.tags.clone().unwrap_or(vec![]);

            // Print out tasks
            println!("");
            println!("{}. {}", i, name.italic());
            println!("{:?}", task.tags);
            println!(
                "   {} | {}",
                task.urgency.to_colored_string(),
                task.status.to_colored_string()
            );
            println!(
                "   Date Added: {}",
                task.date_added.date_naive().to_string().cyan()
            );
            println!("      Description: {}", description.blue());
            println!("      Latest Update: {}", latest.blue());
            match task.completed_on {
                Some(date) => {
                    println!("{}", date.date_naive().to_string().green())
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urgency_ordering() {
        assert!(Urgency::Low < Urgency::Medium);
        assert!(Urgency::Medium < Urgency::High);
        assert!(Urgency::High < Urgency::Critical);
        assert!(Urgency::Low < Urgency::Critical);
        assert!(Urgency::Low == Urgency::Low);
    }

    #[test]
    fn test_sort_by_urgency() {
        let task1 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::Low),
            None,
            None,
        );
        let task2 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::High),
            None,
            None,
        );
        let task3 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::Critical),
            None,
            None,
        );
        let task4 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::Medium),
            None,
            None,
        );
        let task5 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::Low),
            None,
            None,
        );

        let mut task_vec = TaskList::from(vec![task1, task2, task3, task4, task5]);

        // Descending sort
        task_vec.sort_by_urgency(true);
        assert_eq!(task_vec.tasks[0].urgency, Urgency::Critical);
        assert_eq!(task_vec.tasks[1].urgency, Urgency::High);
        assert_eq!(task_vec.tasks[2].urgency, Urgency::Medium);
        assert_eq!(task_vec.tasks[3].urgency, Urgency::Low);
        assert_eq!(task_vec.tasks[4].urgency, Urgency::Low);
        println!("{:?}", task_vec.tasks[3].date_added);
        println!("{:?}", task_vec.tasks[4].date_added);
        assert!(task_vec.tasks[3].date_added > task_vec.tasks[4].date_added);

        // Ascending sort
        task_vec.sort_by_urgency(false);
        assert_eq!(task_vec.tasks[0].urgency, Urgency::Low);
        assert_eq!(task_vec.tasks[1].urgency, Urgency::Low);
        assert_eq!(task_vec.tasks[2].urgency, Urgency::Medium);
        assert_eq!(task_vec.tasks[3].urgency, Urgency::High);
        assert_eq!(task_vec.tasks[4].urgency, Urgency::Critical);
        assert!(task_vec.tasks[0].date_added < task_vec.tasks[1].date_added);
    }
}
