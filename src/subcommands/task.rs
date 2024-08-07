use chrono::prelude::*;
use clap::ValueEnum;
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
    Closed,
}

impl From<&str> for Status {
    fn from(s: &str) -> Self {
        match s {
            "Open" => Status::Open,
            "Working" => Status::Working,
            "Paused" => Status::Paused,
            "Closed" => Status::Closed,
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
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            latest,
            urgency: urgency.unwrap_or(Urgency::Low),
            status: status.unwrap_or(Status::Open),
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

pub fn sort_by_urgency(tasks: &mut Vec<Task>, descending: bool) {
    if descending {
        tasks.sort_by(urgency_desc)
    } else {
        tasks.sort_by(urgency_asc)
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
        let task1 = Task::new(String::from("Task1"), None, None, Some(Urgency::Low), None);
        let task2 = Task::new(String::from("Task1"), None, None, Some(Urgency::High), None);
        let task3 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::Critical),
            None,
        );
        let task4 = Task::new(
            String::from("Task1"),
            None,
            None,
            Some(Urgency::Medium),
            None,
        );
        let task5 = Task::new(String::from("Task1"), None, None, Some(Urgency::Low), None);

        let mut task_vec = vec![task1, task2, task3, task4, task5];

        // Descending sort
        sort_by_urgency(&mut task_vec, true);
        assert_eq!(task_vec[0].urgency, Urgency::Critical);
        assert_eq!(task_vec[1].urgency, Urgency::High);
        assert_eq!(task_vec[2].urgency, Urgency::Medium);
        assert_eq!(task_vec[3].urgency, Urgency::Low);
        assert_eq!(task_vec[4].urgency, Urgency::Low);
        println!("{:?}", task_vec[3].date_added);
        println!("{:?}", task_vec[4].date_added);
        assert!(task_vec[3].date_added > task_vec[4].date_added);

        // Ascending sort
        sort_by_urgency(&mut task_vec, false);
        assert_eq!(task_vec[0].urgency, Urgency::Low);
        assert_eq!(task_vec[1].urgency, Urgency::Low);
        assert_eq!(task_vec[2].urgency, Urgency::Medium);
        assert_eq!(task_vec[3].urgency, Urgency::High);
        assert_eq!(task_vec[4].urgency, Urgency::Critical);
        assert!(task_vec[0].date_added < task_vec[1].date_added);
    }
}
