//! Argument types for the todo_update tool.

use operon_tools_core::{TodoPriority, TodoStatus};
use serde::Deserialize;

/// Arguments for the todo_update tool.
///
/// Specifies the ID of the item to update and optional new values for content, status, and priority.
/// Only provided fields are updated — None means "no change".
#[derive(Debug, Deserialize)]
pub struct TodoUpdateArgs {
    /// Id of the item to update. Required.
    /// Must match an existing item ID (as a string: "1", "2", "3", ...).
    pub id: String,

    /// New content. None = no change.
    /// If provided, must be non-empty after trim.
    #[serde(default)]
    pub content: Option<String>,

    /// New status. None = no change.
    /// Valid values: "pending", "in_progress", "completed".
    #[serde(default)]
    pub status: Option<TodoStatus>,

    /// New priority. None = no change.
    /// Valid values: "high", "medium", "low".
    #[serde(default)]
    pub priority: Option<TodoPriority>,
}
