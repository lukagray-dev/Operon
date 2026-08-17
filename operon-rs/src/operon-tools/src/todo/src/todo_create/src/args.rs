//! Argument types for the todo_create tool.
//!
//! Defines defensive deserialization supporting both single task creation and batch
//! task creation in a single tool call.

use operon_tools_core::TodoPriority;
use serde::Deserialize;

/// A single todo item specification (can be a plain string or an object with content + priority).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TodoCreateItemInput {
    /// Plain string: `"Implement grep tool"`
    String(String),
    /// Object specification: `{ "content": "Implement grep tool", "priority": "high" }`
    Object {
        #[serde(
            alias = "title",
            alias = "task",
            alias = "name",
            alias = "text",
            alias = "description",
            alias = "todo"
        )]
        content: String,
        #[serde(
            default,
            alias = "level",
            alias = "importance",
            alias = "urgency"
        )]
        priority: Option<TodoPriority>,
    },
}

impl TodoCreateItemInput {
    pub fn into_parts(self) -> (String, Option<TodoPriority>) {
        match self {
            Self::String(s) => (s, None),
            Self::Object { content, priority } => (content, priority),
        }
    }
}

/// Flexible arguments for `todo_create`.
///
/// Supports:
/// 1. Single item: `{ "content": "Fix login bug", "priority": "high" }`
/// 2. Batch list: `{ "todos": [ { "content": "Task 1" }, { "content": "Task 2" } ] }`
/// 3. String list: `{ "todos": [ "Task 1", "Task 2" ] }` or `{ "contents": [ "Task 1", "Task 2" ] }`
/// 4. Root array: `[ { "content": "Task 1" }, { "content": "Task 2" } ]` or `[ "Task 1", "Task 2" ]`
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TodoCreateArgs {
    /// Array passed at root: `[ ... ]`
    RootList(Vec<TodoCreateItemInput>),
    /// Standard object payload
    Object(TodoCreateObjectArgs),
}

impl TodoCreateArgs {
    /// Normalizes the deserialized arguments into a list of `(content, priority)` pairs.
    pub fn into_items(self) -> Vec<(String, Option<TodoPriority>)> {
        match self {
            Self::RootList(list) => list.into_iter().map(TodoCreateItemInput::into_parts).collect(),
            Self::Object(obj) => {
                if let Some(todos) = obj.todos {
                    if !todos.is_empty() {
                        return todos.into_iter().map(TodoCreateItemInput::into_parts).collect();
                    }
                }
                if let Some(contents) = obj.contents {
                    if !contents.is_empty() {
                        return contents.into_iter().map(|c| (c, None)).collect();
                    }
                }
                if let Some(content) = obj.content {
                    return vec![(content, obj.priority)];
                }
                Vec::new()
            }
        }
    }
}

/// Object representation for `todo_create` arguments.
#[derive(Debug, Deserialize)]
pub struct TodoCreateObjectArgs {
    /// Single task description.
    #[serde(
        default,
        alias = "title",
        alias = "task",
        alias = "name",
        alias = "text",
        alias = "description",
        alias = "todo"
    )]
    pub content: Option<String>,

    /// Priority level for single task.
    #[serde(
        default,
        alias = "level",
        alias = "importance",
        alias = "urgency"
    )]
    pub priority: Option<TodoPriority>,

    /// Array of items / tasks / todos.
    #[serde(
        default,
        alias = "items",
        alias = "tasks",
        alias = "list"
    )]
    pub todos: Option<Vec<TodoCreateItemInput>>,

    /// Array of string task descriptions.
    #[serde(default)]
    pub contents: Option<Vec<String>>,
}
