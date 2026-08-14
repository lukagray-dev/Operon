//! Argument types for the todo_update tool.
//!
//! Hey friend! Defines the defensive deserialization schema for the todo_update tool's input.
//! Supports numeric and string IDs (`id: 1` vs `id: "1"`), field synonyms, and status aliases.

use operon_tools_core::de::deserialize_flexible_id;
use operon_tools_core::{TodoPriority, TodoStatus};
use serde::Deserialize;

/// Arguments for the todo_update tool.
///
/// Specifies the ID of the item to update and optional new values for content, status, and priority.
/// Only provided fields are updated — None means "no change".
#[derive(Debug, Deserialize)]
pub struct TodoUpdateArgs {
    /// Id of the item to update. Required.
    /// Supports string "1" or numeric integer 1.
    #[serde(
        deserialize_with = "deserialize_flexible_id",
        alias = "todo_id",
        alias = "todoId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: String,

    /// New content. None = no change.
    /// If provided, must be non-empty after trim.
    #[serde(
        default,
        alias = "title",
        alias = "task",
        alias = "name",
        alias = "text",
        alias = "description"
    )]
    pub content: Option<String>,

    /// New status. None = no change.
    /// Valid values: "pending", "in_progress", "completed".
    #[serde(
        default,
        alias = "state"
    )]
    pub status: Option<TodoStatus>,

    /// New priority. None = no change.
    /// Valid values: "high", "medium", "low".
    #[serde(
        default,
        alias = "level",
        alias = "importance"
    )]
    pub priority: Option<TodoPriority>,
}
