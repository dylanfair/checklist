use chrono::prelude::*;
use clap::ValueEnum;
use rusqlite::{types::FromSql, types::ValueRef, ToSql};
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
    pub urgency: Option<Urgency>,
    pub status: Status,
    pub date_added: DateTime<Local>,
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
            urgency,
            status: status.unwrap_or(Status::Open),
            date_added: Local::now(),
            completed_on: None,
        }
    }

    pub fn get_id(&self) -> Uuid {
        self.id
    }

    pub fn from_sql(
        id: Uuid,
        name: String,
        description: Option<String>,
        latest: Option<String>,
        urgency: Option<Urgency>,
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
