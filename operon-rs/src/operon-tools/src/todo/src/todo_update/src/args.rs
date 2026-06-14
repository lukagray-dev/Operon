//! Argument types for the todo_update tool.

use operon_tools_core::{TodoPriority, TodoStatus};
use std::str::FromStr;

/// Arguments for the todo_update tool.
///
/// Constructed via `TodoUpdateArgs::parse` from the raw serde_json::Value attr map.
#[derive(Debug)]
pub struct TodoUpdateArgs {
    /// Id of the item to update. Required.
    /// Must match an existing item ID (as a string: "1", "2", "3", ...).
    pub id: String,

    /// New task description. None = no change.
    /// If provided, must be non-empty after trim.
    pub todo: Option<String>,

    /// New status. None = no change.
    /// Valid values: "pending", "in_progress", "completed".
    pub status: Option<TodoStatus>,

    /// New priority. None = no change.
    /// Valid values: "high", "medium", "low".
    pub priority: Option<TodoPriority>,
}

impl TodoUpdateArgs {
    /// Parses TodoUpdateArgs from the raw args_json Value produced by the dispatcher.
    pub fn parse(args_json: &serde_json::Value) -> Result<TodoUpdateArgs, String> {
        let id = args_json["id"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: id".to_string())?
            .trim()
            .to_string();
        if id.is_empty() {
            return Err("id is empty".to_string());
        }

        let todo = match args_json.get("todo") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                let s = v.as_str().ok_or_else(|| "todo must be a string".to_string())?;
                Some(s.to_string())
            }
        };

        let status = match args_json.get("status") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                if let Some(s) = v.as_str() {
                    TodoStatus::from_str(s).ok()
                } else {
                    None
                }
            }
        };

        let priority = match args_json.get("priority") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                if let Some(s) = v.as_str() {
                    TodoPriority::from_str(s).ok()
                } else {
                    None
                }
            }
        };

        Ok(TodoUpdateArgs {
            id,
            todo,
            status,
            priority,
        })
    }
}
