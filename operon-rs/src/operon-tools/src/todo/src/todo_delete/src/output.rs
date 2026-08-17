//! Output types for the todo_delete tool.

use serde::{Deserialize, Serialize};

/// Output returned to the model after todo item(s) are deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoDeleteOutput {
    /// The list of IDs that were deleted.
    pub ids: Vec<String>,

    /// Primary/first deleted ID (included for backward compatibility).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// List of IDs that were not found in the store, if any.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub not_found: Vec<String>,

    /// Total number of todos remaining after deletion.
    pub remaining: usize,
}
