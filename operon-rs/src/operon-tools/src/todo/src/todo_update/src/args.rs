//! Argument types for the todo_update tool.
//!
//! Defines defensive deserialization supporting both single task updates and batch
//! task updates in a single tool call.

use operon_tools_core::de::{deserialize_flexible_id_opt, deserialize_flexible_string_list_opt};
use operon_tools_core::{TodoPriority, TodoStatus};
use serde::Deserialize;

/// A single item update payload.
#[derive(Debug, Clone, Deserialize)]
pub struct TodoUpdateItemInput {
    /// Id of the item to update.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_id_opt",
        alias = "todo_id",
        alias = "todoId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: Option<String>,

    /// New content. None = no change.
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
    #[serde(
        default,
        alias = "state"
    )]
    pub status: Option<TodoStatus>,

    /// New priority. None = no change.
    #[serde(
        default,
        alias = "level",
        alias = "importance"
    )]
    pub priority: Option<TodoPriority>,
}

/// Flexible arguments for `todo_update`.
///
/// Supports:
/// 1. Single update: `{ "id": "1", "status": "completed" }`
/// 2. Multiple distinct updates: `{ "todos": [ { "id": "1", "status": "completed" }, { "id": "2", "status": "in_progress" } ] }`
/// 3. Bulk update of multiple IDs with shared status/priority: `{ "ids": ["1", "2"], "status": "completed" }`
/// 4. Root array: `[ { "id": "1", ... }, { "id": "2", ... } ]`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TodoUpdateArgs {
    /// Array passed at root: `[ ... ]`
    RootList(Vec<TodoUpdateItemInput>),
    /// Standard object payload
    Object(TodoUpdateObjectArgs),
}

impl TodoUpdateArgs {
    /// Normalizes arguments into a list of `(id, content, status, priority)` update tuples.
    pub fn into_updates(self) -> Vec<(String, Option<String>, Option<TodoStatus>, Option<TodoPriority>)> {
        match self {
            Self::RootList(list) => list
                .into_iter()
                .filter_map(|item| item.id.map(|id| (id, item.content, item.status, item.priority)))
                .collect(),
            Self::Object(obj) => {
                // 1. Batch array of distinct updates
                if let Some(todos) = obj.todos {
                    if !todos.is_empty() {
                        return todos
                            .into_iter()
                            .filter_map(|item| {
                                item.id.map(|id| (id, item.content, item.status, item.priority))
                            })
                            .collect();
                    }
                }

                // 2. Bulk updates sharing the same fields across multiple IDs
                if let Some(ids) = obj.ids {
                    if !ids.is_empty() {
                        return ids
                            .into_iter()
                            .map(|id| (id, obj.content.clone(), obj.status.clone(), obj.priority.clone()))
                            .collect();
                    }
                }

                // 3. Single item update
                if let Some(id) = obj.id {
                    return vec![(id, obj.content, obj.status, obj.priority)];
                }

                Vec::new()
            }
        }
    }
}

/// Object representation for `todo_update` arguments.
#[derive(Debug, Deserialize)]
pub struct TodoUpdateObjectArgs {
    /// Single target ID.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_id_opt",
        alias = "todo_id",
        alias = "todoId",
        alias = "item_id",
        alias = "itemId"
    )]
    pub id: Option<String>,

    /// Array of target IDs when applying bulk updates.
    #[serde(
        default,
        deserialize_with = "deserialize_flexible_string_list_opt",
        alias = "todo_ids",
        alias = "item_ids"
    )]
    pub ids: Option<Vec<String>>,

    /// New content. None = no change.
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
    #[serde(
        default,
        alias = "state"
    )]
    pub status: Option<TodoStatus>,

    /// New priority. None = no change.
    #[serde(
        default,
        alias = "level",
        alias = "importance"
    )]
    pub priority: Option<TodoPriority>,

    /// Array of distinct updates.
    #[serde(
        default,
        alias = "items",
        alias = "tasks",
        alias = "updates"
    )]
    pub todos: Option<Vec<TodoUpdateItemInput>>,
}
