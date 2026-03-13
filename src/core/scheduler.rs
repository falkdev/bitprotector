use crate::db::repository::Repository;

/// Represents a scheduled task type.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskType {
    Sync,
    IntegrityCheck,
}

impl TaskType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskType::Sync => "sync",
            TaskType::IntegrityCheck => "integrity_check",
        }
    }
}

/// Placeholder for scheduler implementation (full implementation in Milestone 7).
pub struct Scheduler {
    repo: std::sync::Arc<Repository>,
}

impl Scheduler {
    pub fn new(repo: std::sync::Arc<Repository>) -> Self {
        Self { repo }
    }
}
