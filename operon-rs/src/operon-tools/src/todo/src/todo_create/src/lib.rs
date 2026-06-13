//! # operon-tools-todo-create
//!
//! Implements the `todo_create` tool for the Operon agent's todo group.
//!
//! Creates a new todo item in the agent's session-scoped task list.
//! Each item is assigned a unique auto-incrementing ID and starts with
//! status "pending" and priority "medium" (or as specified).
//!
//! ## Usage
//!
//! ```rust
//! use operon_tools_todo_create::{definition, execute};
//! use operon_context_normalize::tools::ToolCallId;
//! use operon_tools_core::TodoStore;
//! use serde_json::json;
//!
//! # async fn example() {
//! // 1. Get the tool definition to register with the model
//! let def = definition();
//!
//! // 2. When the model calls the tool, execute it
//! let mut store = TodoStore::new();
//! let args = json!({
//!     "content": "Fix the login bug",
//!     "priority": "high"
//! });
//! let result = execute(
//!     ToolCallId("call_123".to_string()),
//!     args,
//!     &mut store
//! ).await;
//! # }
//! ```

mod args;
mod error;
mod executor;

#[cfg(test)]
mod tests;

pub use args::TodoCreateArgs;
pub use error::TodoCreateToolError;

use operon_context_normalize::tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::{
    emit_tool_progress, TieredToolDefinition, TodoStore, ToolProgress, ToolProgressEmitter,
};

/// Returns the tiered tool definition for the `todo_create` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the most important constraints (call format).
/// - `detailed`: sent after a malformed call. Full explanation with input attrs,
///   output format, error cases, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "todo_create".to_string(),
            description: "Creates a new todo item. Call format: <todo_create todo=\"Fix the login bug\" priority=\"high\"> \
                          `todo` (task description, imperative form) is required. `priority` is optional \
                          (\"high\", \"medium\", \"low\" — default: \"medium\"). Returns a confirmation line and total count."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "todo_create".to_string(),
            description: "\
Creates a new todo item in the agent's session-scoped task list. Each item is assigned a unique \
auto-incrementing ID (as a string: \"1\", \"2\", \"3\", ...) and starts with status \"pending\" \
and priority \"medium\" (or as specified).

## Call format

<todo_create todo=\"Fix the login bug\" priority=\"high\">

All attribute values are strings. The tool tag has no body.

## Attributes

`todo` (required, string, non-empty): Task description. Use imperative form to describe what needs \
to be done. Examples: \"Fix the login bug\", \"Implement the grep tool\", \"Write unit tests for the \
parser\". Whitespace is trimmed — leading and trailing spaces are removed. If empty or whitespace-only, \
returns an error.

`priority` (optional, string): Priority level for the task. Valid values: \"high\", \"medium\", \
\"low\". Defaults to \"medium\" if not provided.

## Output format

Plain text. Shows the created item and updated overall counts:
Created #{id}: {todo} [{priority}]
Total: {total} ({pending} pending, {in_progress} in progress, {completed} completed)

## Status and lifecycle

New items always start with status \"pending\". As work progresses, update the status to \
\"in_progress\" when starting work, and \"completed\" when done. Use the `todo_update` tool to \
change status.

## Session scope

Todos are session-scoped — they exist for the duration of the agent session only.

## Error cases

- Empty todo: \"todo is empty\" — provide a non-empty task description
- Malformed args: \"failed to parse tool arguments: ...\""
                .to_string(),
        },
    }
}

/// Parses `args_json` and executes the todo_create tool.
///
/// Returns a `ToolResult` with plain-text content (ToolContent::Text) on success.
/// Returns `Err(TodoCreateToolError::ArgsParse)` if parsing attributes fails.
///
/// # Arguments
/// - `call_id`: The unique identifier for this tool call.
/// - `args_json`: The raw JSON attribute map.
/// - `store`: Mutable reference to the TodoStore.
pub async fn execute(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
) -> Result<ToolResult, TodoCreateToolError> {
    execute_with_progress(call_id, args_json, store, None).await
}

/// Parses `args_json` and executes the todo_create tool with optional progress reporting.
pub async fn execute_with_progress(
    call_id: ToolCallId,
    args_json: serde_json::Value,
    store: &mut TodoStore,
    progress: Option<ToolProgressEmitter>,
) -> Result<ToolResult, TodoCreateToolError> {
    // Parse the arguments manually. If this fails, return an ArgsParse error.
    let args = TodoCreateArgs::parse(&args_json).map_err(TodoCreateToolError::ArgsParse)?;

    emit_tool_progress(
        progress.as_ref(),
        ToolProgress::running(call_id.clone(), "todo_create", None, "Creating todo item"),
    );

    // Execute the tool and return the result.
    Ok(executor::execute(call_id, args, store).await)
}
