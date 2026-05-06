use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    pub project: String,
    pub status: Status,
    pub parent_id: Option<Uuid>,
    pub children: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub transitions: Vec<Transition>,
    /// Bitmask of flag membership: bit 0 = flag 1, bit 1 = flag 2, bit 2 = flag 3.
    #[serde(default)]
    pub flags: u8,
}

impl Task {
    pub fn new(title: String, project: String, status: Status) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            project,
            status,
            parent_id: None,
            children: Vec::new(),
            created_at: Utc::now(),
            transitions: Vec::new(),
            flags: 0,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn todo_task() -> Task {
        Task::new("task".into(), "".into(), Status::Todo)
    }

    #[test]
    fn status_labels() {
        assert_eq!(Status::Todo.label(), "TODO");
        assert_eq!(Status::Doing.label(), "DOING");
        assert_eq!(Status::Done.label(), "DONE");
    }

    #[test]
    fn task_new_initial_state() {
        let t = todo_task();
        assert_eq!(t.title, "task");
        assert_eq!(t.status, Status::Todo);
        assert!(t.transitions.is_empty());
        assert!(t.parent_id.is_none());
        assert!(t.children.is_empty());
    }

    #[test]
    fn transition_to_updates_status_and_records_history() {
        let mut t = todo_task();
        t.transition_to(Status::Doing);
        assert_eq!(t.status, Status::Doing);
        assert_eq!(t.transitions.len(), 1);
        assert_eq!(t.transitions[0].from, Status::Todo);
        assert_eq!(t.transitions[0].to, Status::Doing);
    }

    #[test]
    fn transition_to_chains_correctly() {
        let mut t = todo_task();
        t.transition_to(Status::Doing);
        t.transition_to(Status::Done);
        assert_eq!(t.status, Status::Done);
        assert_eq!(t.transitions.len(), 2);
        assert_eq!(t.transitions[1].from, Status::Doing);
        assert_eq!(t.transitions[1].to, Status::Done);
    }

    #[test]
    fn time_in_never_visited_status_returns_zero() {
        let t = todo_task();
        assert_eq!(t.time_in(&Status::Done), 0);
        assert_eq!(t.time_in(&Status::Doing), 0);
    }

    #[test]
    fn time_in_completed_span_is_exact() {
        let mut t = todo_task();
        let start = Utc::now() - Duration::seconds(100);
        t.created_at = start;
        t.transitions.push(Transition {
            from: Status::Todo,
            to: Status::Doing,
            at: start + Duration::seconds(60),
        });
        t.status = Status::Doing;
        assert_eq!(t.time_in(&Status::Todo), 60);
    }

    #[test]
    fn time_in_current_status_accumulates() {
        let mut t = todo_task();
        t.created_at = Utc::now() - Duration::seconds(10);
        let secs = t.time_in(&Status::Todo);
        assert!(secs >= 9, "expected >= 9s, got {secs}");
    }

    #[test]
    fn time_in_sums_multiple_spans() {
        let mut t = todo_task();
        let base = Utc::now() - Duration::seconds(300);
        t.created_at = base;
        // Todo 50s → Doing 30s → Todo 40s → Doing (current)
        let t1 = base + Duration::seconds(50);
        let t2 = t1 + Duration::seconds(30);
        let t3 = t2 + Duration::seconds(40);
        t.transitions.push(Transition { from: Status::Todo, to: Status::Doing, at: t1 });
        t.transitions.push(Transition { from: Status::Doing, to: Status::Todo, at: t2 });
        t.transitions.push(Transition { from: Status::Todo, to: Status::Doing, at: t3 });
        t.status = Status::Doing;
        assert_eq!(t.time_in(&Status::Todo), 90);
        // Doing: one completed 30s span + current open span
        let doing = t.time_in(&Status::Doing);
        assert!(doing >= 30);
    }
}
