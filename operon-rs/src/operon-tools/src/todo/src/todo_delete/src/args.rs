//! Argument types for the todo_delete tool.

/// Arguments for the todo_delete tool.
///
/// Constructed via `TodoDeleteArgs::parse` from the raw serde_json::Value attr map.
#[derive(Debug)]
pub struct TodoDeleteArgs {
    /// Id of the item to delete. Required.
    /// Must match an existing item ID (as a string: "1", "2", "3", ...).
    pub id: String,
}

impl TodoDeleteArgs {
    /// Parses TodoDeleteArgs from the raw args_json Value produced by the dispatcher.
    pub fn parse(args_json: &serde_json::Value) -> Result<TodoDeleteArgs, String> {
        let id = args_json["id"]
            .as_str()
            .ok_or_else(|| "missing or non-string attr: id".to_string())?
            .trim()
            .to_string();
        if id.is_empty() {
            return Err("id is empty".to_string());
        }
        Ok(TodoDeleteArgs { id })
    }
}
