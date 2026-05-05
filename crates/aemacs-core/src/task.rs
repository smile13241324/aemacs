use serde::{Deserialize, Serialize};

/// Defines the possible states of a task within a project plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// The task has been defined but work has not yet started.
    Pending,
    /// The task is currently being executed by an agent.
    InProgress,
    /// The task has been successfully finalized.
    Completed,
    /// The task could not be completed as defined.
    Failed,
}

/// Represents a single unit of work within an agent's roadmap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// A human-readable summary of the work to be performed.
    pub description: String,
    /// The current operational state of the task.
    pub status: TaskStatus,
}
