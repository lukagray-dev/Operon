//! Argument types for the todo_delete tool.
//!
//! Defines defensive deserialization supporting both single task deletion and batch
//! task deletion in a single tool call.

use operon_tools_core::de::{deserialize_flexible_id_opt, deserialize_flexible_string_list_opt};
use serde::Deserialize;

/// A single delete item input (either an ID string or an object with an `id` field).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TodoDeleteItemInput {
    /// Plain ID string or number: `"1"` or `1`
    String(#[serde(deserialize_with = "operon_tools_core::de::deserialize_flexible_id")] String),
    /// Object with an ID field: `{ "id": "1" }`
    Object {
        #[serde(
            deserialize_with = "operon_tools_core::de::deserialize_flexible_id",
            alias = "todo_id",
            alias = "todoId",
            alias = "item_id",
            alias = "itemId"
        )]
        id: String,
    },
}

impl TodoDeleteItemInput {
    pub fn into_id(self) -> String {
        match self {
            Self::String(s) => s,
            Self::Object { id } => id,
        }
    }
}

/// Flexible arguments for `todo_delete`.
///
/// Supports:
/// 1. Single ID: `{ "id": "1" }`
/// 2. Batch list of IDs: `{ "ids": ["1", "2", "3"] }`
/// 3. Array of objects: `{ "todos": [ { "id": "1" }, { "id": "2" } ] }`
/// 4. Root array: `["1", "2", "3"]` or `[ { "id": "1" }, { "id": "2" } ]`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TodoDeleteArgs {
    /// Array passed at root: `["1", "2"]` or `[ { "id": "1" } ]`
    RootList(Vec<TodoDeleteItemInput>),
    /// Standard object payload
    Object(TodoDeleteObjectArgs),
}

impl TodoDeleteArgs {
    /// Normalizes arguments into a list of target ID strings.
    pub fn into_ids(self) -> Vec<String> {
        match self {
            Self::RootList(list) => list.into_iter().map(TodoDeleteItemInput::into_id).collect(),
            Self::Object(obj) => {
                if let Some(ids) = obj.ids {
                    if !ids.is_empty() {
                        return ids;
                    }
                }
                if let Some(todos) = obj.todos {
                    if !todos.is_empty() {
                        return todos
                            .into_iter()
                            .map(TodoDeleteItemInput::into_id)
                            .collect();
                    }
                }
                if let Some(id) = obj.id {
                    return vec![id];
                }
                Vec::new()
            }
        }
    }
}

/// Object representation for `todo_delete` arguments.
#[derive(Debug, Deserialize)]
pub struct TodoDeleteObjectArgs {
    /// Single item ID to delete.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_id_opt",
        alias = "todo_id",
        alias = "todoId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: Option<String>,

    /// Array of item IDs to delete.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_string_list_opt",
        alias = "todo_ids",
        alias = "item_ids"
    )]
    pub ids: Option<Vec<String>>,

    /// Array of objects or strings to delete.
    #[serde(default, alias = "items", alias = "tasks")]
    pub todos: Option<Vec<TodoDeleteItemInput>>,
}
