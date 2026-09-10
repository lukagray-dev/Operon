// args.rs — Argument types and deserialization logic for the `spawn_agent` tool.
//
// Hey friend! Welcome to the subagent tool args module!
// When the parent AI model decides it needs help with a task (or wants to parallelize work),
// it calls the `spawn_agent` tool.
//
// The model sends a JSON payload with:
// 1. `task`: The prompt or job description the subagent must execute.
// 2. `role` (optional): A persona or specialization like "researcher", "tester", "reviewer", or "coder".
// 3. `description` (optional): A concise 1-line summary of what the subagent is doing.
//
// This module deserializes that JSON payload into a strongly-typed Rust struct `SpawnAgentArgs`,
// validating required fields so that our backend runtime gets clean, safe data to work with.

use serde::{Deserialize, Serialize};

use crate::error::SpawnAgentToolError;

/// Input arguments passed by the AI model when calling the `spawn_agent` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnAgentArgs {
    /// The explicit instruction or task prompt that the subagent should execute.
    ///
    /// For example:
    /// "Analyze the performance of database queries in src/db.rs and suggest optimizations."
    pub task: String,

    /// Optional specialization or role for the subagent.
    ///
    /// Examples: "researcher", "coder", "tester", "reviewer", "architect".
    /// If not provided, defaults to "general".
    #[serde(default = "default_role")]
    pub role: String,

    /// Optional short description explaining the goal of this subagent.
    ///
    /// This is useful for status bars, logs, and user-facing notifications.
    #[serde(default)]
    pub description: Option<String>,
}

/// Fallback role name when the model does not specify one.
fn default_role() -> String {
    "general".to_string()
}

impl SpawnAgentArgs {
    /// Deserializes a `serde_json::Value` into `SpawnAgentArgs`.
    ///
    /// # Errors
    ///
    /// Returns [`SpawnAgentToolError::ArgsParse`] if required fields are missing
    /// (e.g. `task` is not provided) or if types do not match.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, SpawnAgentToolError> {
        serde_json::from_value(value.clone()).map_err(|e| SpawnAgentToolError::ArgsParse(e.to_string()))
    }
}

