//! Output types for the todo_delete tool.

use serde::{Deserialize, Serialize};

/// Output returned to the model after a todo item is deleted.
///
/// Contains the ID that was deleted and the remaining count of todos.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodoDeleteOutput {
    /// The id that was deleted.
    pub id: String,

    /// Total number of todos remaining after deletion.
    pub remaining: usize,
}
