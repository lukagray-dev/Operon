//! Output types for the todo_update tool.

use operon_tools_core::TodoItem;
use serde::{Deserialize, Serialize};

/// Output returned to the model after todo item(s) are updated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoUpdateOutput {
    /// The list of updated items with all current field values.
    pub items: Vec<TodoItem>,

    /// Primary/first updated item (included for backward compatibility).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<TodoItem>,

    /// List of IDs that were not found in the store, if any.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub not_found: Vec<String>,
}
