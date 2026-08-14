//! Argument types for the todo_delete tool.
//!
//! Hey friend! Defines the defensive deserialization schema for the todo_delete tool's input.
//! Supports string and integer IDs (`id: 1` vs `id: "1"`) and ID field aliases.

use operon_tools_core::de::deserialize_flexible_id;
use serde::Deserialize;

/// Arguments for the todo_delete tool.
///
/// Specifies the ID of the item to delete.
#[derive(Debug, Deserialize)]
pub struct TodoDeleteArgs {
    /// Id of the item to delete. Required.
    /// Supports string "1" or integer 1.
    #[serde(
        deserialize_with = "deserialize_flexible_id",
        alias = "todo_id",
        alias = "todoId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: String,
}
