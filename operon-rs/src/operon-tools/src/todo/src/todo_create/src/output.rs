//! Output types for the todo_create tool.
//!
//! This module defines the structured result format returned by the todo_create tool
//! on successful completion.

use operon_tools_core::TodoItem;
use serde::{Deserialize, Serialize};

/// Output returned to the model after a todo item is created.
///
/// Contains the newly created item (including its auto-assigned ID) and the
/// total count of todos in the store after creation.
#[derive(Debug, Serialize, Deserialize)]
pub struct TodoCreateOutput {
    /// The created item, including its assigned id.
    /// The id is a string representation of an auto-incrementing integer: "1", "2", "3", ...
    pub item: TodoItem,

    /// Total number of todos in the store after creation.
    /// Useful for the model to understand the current task list size.
    pub total: usize,
}
