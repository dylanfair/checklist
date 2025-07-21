use std::cmp::Ordering;
use std::collections::HashSet;
use std::string::ToString;

use chrono::prelude::*;
use clap::ValueEnum;
use crossterm::style::Stylize;
use ratatui::widgets::ListState;
use rusqlite::{ToSql, types::FromSql, types::ValueRef};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Enum to help control what tasks are to be displayed
#[derive(Clone, Copy, Debug, ValueEnum, strum_macros::Display, Serialize, Deserialize)]
pub enum Display {
    All,
    Completed,
    NotCompleted,
}

impl Display {
    /// Will rotate through the different enum variants
    pub fn next(&mut self) {
        match self {
            Display::All => *self = Display::Completed,
            Display::Completed => *self = Display::NotCompleted,
            Display::NotCompleted => *self = Display::All,
        }
    }
}

/// Enum to handle the urgency of a `Task`
#[derive(
    Clone,
    Debug,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    ValueEnum,
    strum_macros::Display,
    Default,
    Serialize,
    Deserialize,
)]
pub enum Urgency {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl Urgency {
    /// Will return a `StyledContent<String>` based on the Urgency
    pub fn to_colored_string(self) -> crossterm::style::StyledContent<String> {
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

/// Enum to handle the status of a `Task`
#[derive(Clone, Debug, Copy, ValueEnum, strum_macros::Display, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Open,
    Working,
    Paused,
    Completed,
}

impl Status {
    /// Will return a `StyledContent<String>` based on the Status
    pub fn to_colored_string(self) -> crossterm::style::StyledContent<String> {
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

/// Struct that holds the attributes to a Task
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub latest: Option<String>,
    pub urgency: Urgency,
    pub status: Status,
    pub tags: Option<HashSet<String>>,
    pub date_added: DateTime<Local>,
    pub completed_on: Option<DateTime<Local>>,
}

impl Task {
    /// Creates a new `Task`, requiring only a `String` name.
    /// Everything else is optional.
    pub fn new(
        name: String,
        description: Option<String>,
        latest: Option<String>,
        urgency: Option<Urgency>,
        status: Option<Status>,
        tags: Option<HashSet<String>>,
    ) -> Self {
        let status_value = status.unwrap_or(Status::Open);
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            latest,
            urgency: urgency.unwrap_or(Urgency::Low),
            status: status_value,
            tags,
            date_added: Local::now(),
            completed_on: if status_value == Status::Completed {
                Some(Local::now())
            } else {
                None
            },
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
        tags: Option<HashSet<String>>,
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

/// Struct that holds a vector of `Task`, and
/// a ratatui's `ListState`.
///
/// Meant to be used within the TUI
#[derive(Clone, Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
    pub state: ListState,
}

impl TaskList {
    /// Creates a new `TaskList` with an empty vector of `Task`s and a `ListState::default()`.
    pub fn new() -> Self {
        TaskList {
            tasks: vec![],
            state: ListState::default(),
        }
    }

    /// Creates a new `TaskList` given a vector of `Task`s. Will start with `ListState::default()`.
    pub fn from(tasks: Vec<Task>) -> Self {
        TaskList {
            tasks,
            state: ListState::default(),
        }
    }

    /// Sorts the `TaskList` based on the `Urgency` in the vector of `Task`s.
    /// If `descending` is true, sort will be done in a Critical > Low order.
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

    /// Filters the `TaskList`, either on a `Display` given or by a tag `String`
    pub fn filter_tasks(&mut self, display_option: Option<Display>, tags_filter: String) {
        let mut tasks_to_keep = vec![];
        'task: for task in &mut self.tasks.iter() {
            // check if fits our display needs
            match display_option {
                Some(display) => match display {
                    Display::Completed => {
                        if task.status != Status::Completed {
                            continue 'task;
                        }
                    }
                    Display::NotCompleted => {
                        if task.status == Status::Completed {
                            continue 'task;
                        }
                    }
                    Display::All => {}
                },
                None => {
                    if task.status == Status::Completed {
                        continue 'task;
                    }
                }
            }

            // Check if our tag string is within any of our tags
            if !tags_filter.is_empty() {
                match task.tags.clone() {
                    Some(task_tags) => {
                        for tag in task_tags {
                            if tag.contains(&tags_filter) {
                                // on first match, we can add to our tasks
                                // to keep and move on
                                tasks_to_keep.push(task.clone());
                                continue 'task;
                            }
                        }
                        continue 'task;
                    }
                    None => continue 'task,
                }
            } else {
                // if tags_filter is empty, we just push everything
                tasks_to_keep.push(task.clone());
            }
        }
        self.tasks = tasks_to_keep;
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
