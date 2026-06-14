//! Argument types for the todo_create tool.
//!
//! This module defines manual parsing logic for the todo_create tool's plain-text
//! attr-based input format. All attribute values arrive as strings from the custom
//! LLM tool-call parser — no serde Deserialize is used.
//!
//! Call format:
//!   <todo_create todo="Fix the login bug" priority="high">
//!
//! `todo` (required, string, non-empty after trim) — was `content`.
//! `priority` (optional string: "high"|"medium"|"low", default "medium").

use operon_tools_core::TodoPriority;
use std::str::FromStr;

/// Arguments for the todo_create tool.
///
/// Constructed via `TodoCreateArgs::parse` from the raw serde_json::Value attr map.
#[derive(Debug)]
pub struct TodoCreateArgs {
    /// The task description. Use imperative form: "Implement the grep tool".
    /// Should be concise but descriptive enough to understand the task.
    /// Validation: must be non-empty after trim.
    pub todo: String,

    /// Priority level. Defaults to medium if not provided.
    /// Valid values: "high", "medium", "low".
    /// Determines the urgency and ordering of the task.
    pub priority: Option<TodoPriority>,
}

impl TodoCreateArgs {
    /// Parses TodoCreateArgs from the raw args_json Value produced by the dispatcher.
    pub fn parse(args_json: &serde_json::Value) -> Result<TodoCreateArgs, String> {
        let todo = args_json["todo"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: todo".to_string())?
            .trim()
            .to_string();
        if todo.is_empty() {
            return Err("todo is empty".to_string());
        }
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
        Ok(TodoCreateArgs { todo, priority })
    }
}
