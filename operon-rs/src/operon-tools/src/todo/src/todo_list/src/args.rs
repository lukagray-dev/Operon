//! Argument types for the todo_list tool.

use operon_tools_core::{TodoPriority, TodoStatus};
use std::str::FromStr;

/// Arguments for the todo_list tool.
///
/// Constructed via `TodoListArgs::parse` from the raw serde_json::Value attr map.
#[derive(Debug)]
pub struct TodoListArgs {
    /// Optional filter by status. If None, returns all todos regardless of status.
    /// Valid values: "pending", "in_progress", "completed".
    pub status: Option<TodoStatus>,

    /// Optional filter by priority. If None, returns all todos regardless of priority.
    /// Valid values: "high", "medium", "low".
    pub priority: Option<TodoPriority>,
}

impl TodoListArgs {
    /// Parses TodoListArgs from the raw args_json Value produced by the dispatcher.
    pub fn parse(args_json: &serde_json::Value) -> Result<TodoListArgs, String> {
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

        Ok(TodoListArgs { status, priority })
    }
}
