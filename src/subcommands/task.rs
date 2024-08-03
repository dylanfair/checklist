use chrono::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum Urgency {
    Low,
    Medium,
    High,
    Critical,
}

pub enum Status {
    Open,
    Working,
    Paused,
    Closed,
}

pub struct Task {
    id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub urgency: Option<Urgency>,
    pub status: Status,
    pub completed_on: Option<DateTime<Local>>,
}

impl Task {
    pub fn new(name: String, description: Option<String>, urgency: Option<Urgency>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            urgency,
            status: Status::Open,
            completed_on: None,
        }
    }
}
