//! # operon-tools-subagent-spawn
//!
//! Implements the `spawn_agent` tool definition and types for the Operon subagent group.
//!
//! ## Overview
//!
//! The `spawn_agent` tool lets the parent AI model spawn one or multiple autonomous
//! subagents, each running with its own dedicated session, workspace access, and tools.
//! Multiple subagents execute concurrently in background tasks. When all subagents finish
//! their work, their final responses are returned to the parent model so it can incorporate
//! their findings and continue its work.
//!
//! ## Execution Architecture
//!
//! Just like `ask`, `spawn_agent` is intercepted and orchestrated by the session runner
//! (`operon-session`). The session runner owns the provider configuration, workspace root,
//! session store persistence, and event channels.
//!
//! This crate provides:
//! - [`SpawnAgentArgs`] — Argument deserialization from the model's tool call JSON.
//! - [`SpawnAgentOutput`] — Completion result returned back to the parent agent.
//! - [`SpawnAgentToolError`] — Parse error type.
//! - [`definition()`] — Canonical `ToolDefinition` exposed to LLM function calling.

mod args;
mod error;
mod output;

#[cfg(test)]
mod tests;

pub use args::SpawnAgentArgs;
pub use error::SpawnAgentToolError;
pub use output::SpawnAgentOutput;

use operon_context_normalize_tools::ToolDefinition;
use serde_json::json;

/// Returns the canonical tool definition for the `spawn_agent` tool.
///
/// Follows industry standards for tool/function calling (OpenAI, Anthropic, Gemini):
/// - Required `task` parameter defining the goal/instructions for the subagent.
/// - Optional `role` parameter defining the persona/specialization (e.g. "researcher", "tester").
/// - Optional `description` parameter summarizing the subagent's objective.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the JSON Schema for spawn_agent arguments here.
    let parameters = json!({
        "type": "object",
        "required": ["task"],
        "properties": {
            "task": {
                "type": "string",
                "minLength": 1,
                "description": "Clear and detailed instructions or goal for the subagent to accomplish autonomously."
            },
            "role": {
                "type": "string",
                "description": "Optional persona or specialization for the subagent (e.g., 'researcher', 'coder', 'tester', 'reviewer', 'analyst'). Defaults to 'general'."
            },
            "description": {
                "type": "string",
                "description": "Optional concise one-line summary describing the purpose of this subagent."
            }
        }
    });

    ToolDefinition {
        name: "spawn_agent".to_string(),
        description: "Spawns an autonomous subagent with its own dedicated session to execute a specific task in parallel. \
                      You can call this multiple times to spawn multiple subagents concurrently. \
                      Once all subagents complete their tasks, their final responses will be sent back to you so you can continue your work."
            .to_string(),
        parameters,
    }
}

