//! Argument types for the todo_delete tool.

use serde::Deserialize;

/// Arguments for the todo_delete tool.
///
/// Specifies the ID of the item to delete.
#[derive(Debug, Deserialize)]
pub struct TodoDeleteArgs {
    /// Id of the item to delete. Required.
    /// Must match an existing item ID (as a string: "1", "2", "3", ...).
    pub id: String,
}
