use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Todo,
    Doing,
    Done,
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Todo => "TODO",
            Status::Doing => "DOING",
            Status::Done => "DONE",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub from: Status,
    pub to: Status,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub project: String,
    pub status: Status,
    pub parent_id: Option<Uuid>,
    pub children: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub transitions: Vec<Transition>,
}

impl Task {
    pub fn new(title: String, project: String, status: Status) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            description: None,
            project,
            status,
            parent_id: None,
            children: Vec::new(),
            created_at: Utc::now(),
            transitions: Vec::new(),
        }
    }

    pub fn transition_to(&mut self, new_status: Status) {
        let old = self.status.clone();
        self.transitions.push(Transition {
            from: old,
            to: new_status.clone(),
            at: Utc::now(),
        });
        self.status = new_status;
    }

    pub fn time_in(&self, status: &Status) -> i64 {
        let mut total = 0i64;
        let init_status = if self.transitions.is_empty() {
            &self.status
        } else {
            &self.transitions[0].from
        };
        let mut enter: Option<DateTime<Utc>> = if init_status == status {
            Some(self.created_at)
        } else {
            None
        };
        let mut cur_status = init_status.clone();

        for t in &self.transitions {
            if t.from == *status {
                if let Some(start) = enter.take() {
                    total += (t.at - start).num_seconds();
                }
            }
            if t.to == *status {
                enter = Some(t.at);
            }
            cur_status = t.to.clone();
        }

        if cur_status == *status {
            if let Some(start) = enter {
                total += (Utc::now() - start).num_seconds();
            }
        }

        total
    }
}
