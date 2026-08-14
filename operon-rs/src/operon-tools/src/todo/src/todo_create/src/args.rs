//! Argument types for the todo_create tool.
//!
//! Hey friend! Defines the defensive deserialization schema for the todo_create tool's input.
//! Supports common task title/content aliases and priority synonyms.

use operon_tools_core::TodoPriority;
use serde::Deserialize;

/// Arguments for the todo_create tool.
///
/// Specifies a task description (required) and an optional priority level.
/// The task description should be in imperative form (e.g., "Fix the login bug").
#[derive(Debug, Deserialize)]
pub struct TodoCreateArgs {
    /// The task description. Use imperative form: "Implement the grep tool".
    /// Should be concise but descriptive enough to understand the task.
    /// Validation: must be non-empty after trim.
    #[serde(
        alias = "title",
        alias = "task",
        alias = "name",
        alias = "text",
        alias = "description",
        alias = "todo"
    )]
    pub content: String,

    /// Priority level. Defaults to medium if not provided.
    /// Valid values: "high", "medium", "low".
    /// Determines the urgency and ordering of the task.
    #[serde(
        default,
        alias = "level",
        alias = "importance",
        alias = "urgency"
    )]
    pub priority: Option<TodoPriority>,
}
