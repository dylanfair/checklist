use chrono::prelude::*;
use clap::ValueEnum;
use crossterm::style::Stylize;
use rusqlite::{types::FromSql, types::ValueRef, ToSql};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::mem::swap;
use std::string::ToString;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, ValueEnum, strum_macros::Display)]
pub enum Display {
    All,
    Completed,
    NotCompleted,
}

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

#[derive(Clone, Debug, Copy, ValueEnum, strum_macros::Display, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub latest: Option<String>,
    pub urgency: Urgency,
    pub status: Status,
    pub tags: Option<HashSet<String>>,
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

    #[warn(dead_code)]
    pub fn update(
        &mut self,
        name: Option<String>,
        description: Option<String>,
        latest: Option<String>,
        urgency: Option<Urgency>,
        status: Option<Status>,
        add_tags: Option<HashSet<String>>,
        remove_tags: Option<HashSet<String>>,
    ) {
        if let Some(n) = name {
            self.name = n;
        }
        // Description
        self.description = description;
        // Latest
        self.latest = latest;
        // Urgency
        if let Some(urg) = urgency {
            self.urgency = urg;
        }
        // Status
        if let Some(stat) = status {
            self.status = stat;
            if self.status == Status::Completed {
                self.completed_on = Some(Local::now());
            } else {
                self.completed_on = None;
            }
        }
        if let Some(add_t) = add_tags {
            let mut old_tags: Option<HashSet<String>> = Some(HashSet::new());
            swap(&mut old_tags, &mut self.tags);
            let mut tags = old_tags.unwrap_or(HashSet::new());
            tags.extend(add_t);
            self.tags = Some(tags);
        }
        if self.tags.is_some() {
            if let Some(remove_t) = remove_tags {
                let mut old_tags: Option<HashSet<String>> = Some(HashSet::new());
                swap(&mut old_tags, &mut self.tags);
                let mut tags = old_tags.unwrap();

                for t in remove_t {
                    tags.remove(&t);
                }
                self.tags = Some(tags);
            }
        }
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

#[derive(Clone, Debug)]
pub struct TaskList {
    pub tasks: Vec<Task>,
}

impl TaskList {
    pub fn new() -> Self {
        TaskList { tasks: vec![] }
    }

    #[warn(dead_code)]
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

    #[warn(dead_code)]
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn filter_tasks(
        &mut self,
        display_option: Option<Display>,
        tags_option: Option<Vec<String>>,
    ) {
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

            // Check if it has tags we are looking for
            if let Some(tags) = tags_option.clone() {
                match task.tags.clone() {
                    Some(task_tags) => {
                        for t in tags {
                            if task_tags.contains(&t) == false {
                                continue 'task;
                            }
                        }
                    }
                    None => continue 'task,
                }
            }
            tasks_to_keep.push(task.clone());
        }
        self.tasks = tasks_to_keep;
    }

    pub fn display_tasks(&self) {
        for task in self.tasks.iter() {
            let name = task.name.clone();
            let description = task.description.clone().unwrap_or(String::from("None"));
            let latest = task.latest.clone().unwrap_or(String::from("None"));
            let task_tags = task.tags.clone().unwrap_or(HashSet::new());

            // Print out tasks
            println!("");
            println!("{}", name.underlined());
            print!(" Tags:");
            for tag in task_tags {
                print!(" {}", tag.blue());
            }
            print!("\n");
            print!(
                "   {} | {}",
                task.urgency.to_colored_string(),
                task.status.to_colored_string()
            );
            match task.completed_on {
                Some(date) => {
                    print!(" - {}", date.date_naive().to_string().green())
                }
                None => {}
            }
            print!("\n");
            println!(
                "   Date Added: {}",
                task.date_added.date_naive().to_string().cyan()
            );
            println!("  Description: {}", description.blue());
            println!("  Latest Update: {}", latest.blue());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_update() {
        let mut task = Task::new(String::from("Test task"), None, None, None, None, None);
        // add tags
        task.update(
            None,
            None,
            None,
            Some(Urgency::Low),
            Some(Status::Completed),
            Some(HashSet::from([
                "task1".to_string(),
                "task2".to_string(),
                "task1".to_string(),
            ])),
            None,
        );
        assert_eq!(
            task.tags,
            Some(HashSet::from(["task1".to_string(), "task2".to_string()]))
        );
        assert_eq!(task.urgency, Urgency::Low);
        assert_eq!(task.status, Status::Completed);
        assert!(task.completed_on.is_some());

        // remove tags
        task.update(
            None,
            None,
            None,
            Some(Urgency::High),
            Some(Status::Paused),
            None,
            Some(HashSet::from(["task1".to_string()])),
        );
        assert_eq!(task.tags, Some(HashSet::from(["task2".to_string()])));
        assert_eq!(task.urgency, Urgency::High);
        assert_eq!(task.status, Status::Paused);
        assert!(task.completed_on.is_none());
    }

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
