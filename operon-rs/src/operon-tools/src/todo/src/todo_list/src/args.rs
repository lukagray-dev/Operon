//! Argument types for the todo_list tool.

use operon_tools_core::{TodoPriority, TodoStatus};
use serde::Deserialize;

/// Arguments for the todo_list tool.
///
/// Specifies optional filters for status and priority.
/// If both are None, all todos are returned.
#[derive(Debug, Deserialize)]
pub struct TodoListArgs {
    /// Optional filter by status. If None, returns all todos regardless of status.
    /// Valid values: "pending", "in_progress", "completed".
    #[serde(default)]
    pub status: Option<TodoStatus>,

    /// Optional filter by priority. If None, returns all todos regardless of priority.
    /// Valid values: "high", "medium", "low".
    #[serde(default)]
    pub priority: Option<TodoPriority>,
}
