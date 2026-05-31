//! Output types for the todo_update tool.

use operon_tools_core::TodoItem;
use serde::{Deserialize, Serialize};

/// Output returned to the model after a todo item is updated.
///
/// Contains the updated item with all current field values.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodoUpdateOutput {
    /// The updated item with all current field values.
    pub item: TodoItem,
}
