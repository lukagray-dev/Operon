//! Output types for the todo_list tool.

use operon_tools_core::TodoItem;
use serde::{Deserialize, Serialize};

/// Output returned to the model after listing todos.
///
/// Contains the filtered list of items and counts by status for quick overview.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodoListOutput {
    /// The filtered list of todo items (or all items if no filters applied).
    pub items: Vec<TodoItem>,

    /// Total number of todos in the store (unfiltered count).
    /// Useful for understanding the full scope of work.
    pub total: usize,

    /// Count of items with status "pending" (unfiltered).
    pub pending: usize,

    /// Count of items with status "in_progress" (unfiltered).
    pub in_progress: usize,

    /// Count of items with status "completed" (unfiltered).
    pub completed: usize,
}
