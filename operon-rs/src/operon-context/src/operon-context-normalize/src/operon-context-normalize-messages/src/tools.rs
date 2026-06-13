//! Canonical tool-call and tool-result types used throughout Operon.

use serde::{Deserialize, Serialize};

/// A strongly-typed wrapper around a tool-call identifier string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolCallId(pub String);

/// A single tool call parsed from the model's text response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this call. Used to pair the call with its [`ToolResult`].
    pub id: ToolCallId,

    /// The exact name of the tool the model wants to invoke.
    pub name: String,

    /// The arguments parsed for the tool (keys and values are string values).
    pub arguments: serde_json::Value,
}

/// The content payload carried by a [`ToolResult`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolContent {
    /// Plain UTF-8 text returned by the tool.
    Text(String),
}

/// The outcome of executing a tool call, ready to be inserted back into context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// The `id` of the [`ToolCall`] this result is answering.
    pub call_id: ToolCallId,

    /// The name of the tool that produced this result.
    pub name: String,

    /// The content returned by the tool execution.
    pub content: ToolContent,

    /// Whether the tool execution produced an error.
    pub is_error: bool,

    /// Paths successfully read by the read tool. Used by the dispatcher to update
    /// the read ledger for read-before-write enforcement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_paths: Option<Vec<String>>,
}

/// A description of a tool that the model may call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The unique name for this tool.
    pub name: String,

    /// A description of what the tool does, including its plain-text calling syntax.
    pub description: String,
}
