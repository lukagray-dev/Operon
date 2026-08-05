//! Argument types for the todo_create tool.
//!
//! This module defines the deserialization schema for the todo_create tool's input.
//! The tool accepts a task description and an optional priority level.

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
    pub content: String,

    /// Priority level. Defaults to medium if not provided.
    /// Valid values: "high", "medium", "low".
    /// Determines the urgency and ordering of the task.
    #[serde(default)]
    pub priority: Option<TodoPriority>,
}
