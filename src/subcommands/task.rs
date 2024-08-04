use chrono::prelude::*;
use clap::ValueEnum;
use rusqlite::{types::FromSql, ToSql};
use std::string::ToString;
use uuid::Uuid;

#[derive(Clone, Debug, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum, strum_macros::Display)]
pub enum Urgency {
    Low,
    Medium,
    High,
    Critical,
}

impl ToSql for Urgency {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

#[derive(Clone, Debug, Copy, strum_macros::Display)]
pub enum Status {
    Open,
    Working,
    Paused,
    Closed,
}

impl ToSql for Status {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(self.to_string().into())
    }
}

impl FromSql for Status {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            b"Open" => Ok(Self::Open),
            b"Working" => Ok(Self::Working),
            b"Paused" => Ok(Self::Paused),
            b"Closed" => Ok(Self::Closed),
        }
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
    pub completed_on: Option<DateTime<Local>>,
}

impl Task {
    pub fn new(
        name: String,
        description: Option<String>,
        latest: Option<String>,
        urgency: Option<Urgency>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            latest,
            urgency,
            status: Status::Open,
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
        completed_on: Option<DateTime<Local>>,
    ) -> Self {
        Self {
            id,
            name,
            description,
            latest,
            urgency,
            status,
            completed_on,
        }
    }
}
