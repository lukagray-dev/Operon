//! Output types for the todo_create tool.

use operon_tools_core::TodoItem;
use serde::{Deserialize, Serialize};

/// Output returned to the model after todo item(s) are created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoCreateOutput {
    /// List of all created items in order of creation.
    pub items: Vec<TodoItem>,

    /// Primary/first created item (included for backward compatibility).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<TodoItem>,

    /// Total number of todos in the store after creation.
    pub total: usize,
}
