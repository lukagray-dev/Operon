//! Shared todo types used across all todo tool sub-crates and the dispatcher.
//!
//! Defines the core data structures for the agent's task list: TodoItem, TodoStatus,
//! and TodoPriority. These types are serialized/deserialized by all four todo tools
//! (create, list, update, delete) and are stored in the session-scoped TodoStore.

use serde::{Deserialize, Serialize};

/// Priority level of a todo item.
///
/// Used to categorize tasks by urgency. Defaults to Medium if not specified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    /// High priority — urgent, should be done first.
    High,
    /// Medium priority — normal, default level.
    Medium,
    /// Low priority — can be deferred.
    Low,
}

impl Default for TodoPriority {
    fn default() -> Self {
        TodoPriority::Medium
    }
}

impl std::str::FromStr for TodoPriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "high" => Ok(TodoPriority::High),
            "medium" => Ok(TodoPriority::Medium),
            "low" => Ok(TodoPriority::Low),
            _ => Err(format!("invalid priority value: '{}'", s)),
        }
    }
}

impl std::fmt::Display for TodoPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TodoPriority::High => "high",
            TodoPriority::Medium => "medium",
            TodoPriority::Low => "low",
        };
        write!(f, "{}", s)
    }
}

/// Status of a todo item.
///
/// Tracks the lifecycle of a task from creation through completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    /// Task has not been started yet. Default status on creation.
    Pending,
    /// Task is currently being worked on.
    InProgress,
    /// Task has been completed.
    Completed,
}

impl Default for TodoStatus {
    fn default() -> Self {
        TodoStatus::Pending
    }
}

impl std::str::FromStr for TodoStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TodoStatus::Pending),
            "in_progress" => Ok(TodoStatus::InProgress),
            "completed" => Ok(TodoStatus::Completed),
            _ => Err(format!("invalid status value: '{}'", s)),
        }
    }
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
        };
        write!(f, "{}", s)
    }
}

/// A single todo item.
///
/// Represents one task in the agent's session-scoped task list.
/// Items are created with auto-assigned numeric IDs (as strings: "1", "2", "3", ...).
/// Status and priority can be updated as work progresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// Unique identifier. Auto-assigned by `TodoStore` on creation.
    /// Format: simple incrementing integer as string: "1", "2", "3", ...
    pub id: String,

    /// The task description. Use imperative form: "Implement the grep tool".
    /// Should be concise but descriptive enough to understand the task.
    pub content: String,

    /// Current status. Defaults to `pending` on creation.
    /// Transitions: pending → in_progress → completed.
    pub status: TodoStatus,

    /// Priority level. Defaults to `medium` on creation.
    /// Used to organize and prioritize work.
    pub priority: TodoPriority,
}
