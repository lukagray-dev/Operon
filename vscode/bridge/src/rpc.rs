//! JSON-RPC protocol types for the operon-vscode-bridge stdio interface.
//!
//! These types mirror the TypeScript definitions in `extension/src/rpc.ts`.
//! Any change here must be reflected there and vice versa.

// All types here are intentionally defined ahead of full handler implementation.
// Suppress dead_code lints during the scaffold phase — they will all be used
// once handler.rs wires up the SessionRunner.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ── Requests (Extension → Bridge) ────────────────────────────────────────────

/// A single JSON-RPC request sent by the VS Code extension over stdin.
#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    /// Unique monotonic ID assigned by the extension. All events emitted in
    /// response to this request carry the same ID.
    pub id: u64,

    /// The method to invoke.
    pub method: RpcMethod,

    /// Method-specific parameters (raw JSON object).
    pub params: serde_json::Value,
}

/// All methods the bridge understands.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcMethod {
    /// Submit a user prompt to the agent loop.
    SubmitPrompt,
    /// Cancel the currently running prompt.
    Cancel,
    /// Approve a pending tool permission request.
    ApprovePermission,
    /// Deny a pending tool permission request.
    DenyPermission,
    /// Load and return the message history for a session.
    LoadHistory,
}

/// Parameters for the `submit_prompt` method.
#[derive(Debug, Deserialize)]
pub struct SubmitPromptParams {
    /// Existing session ID to continue, or `None` to start a new session.
    pub session_id: Option<String>,
    /// The user's prompt text.
    pub prompt: String,
    /// Absolute path to the workspace root the agent should operate in.
    pub workspace_path: Option<String>,
}

/// Parameters for the `approve_permission` and `deny_permission` methods.
#[derive(Debug, Deserialize)]
pub struct PermissionDecisionParams {
    /// The `permission_id` from the original `permission_req` event.
    pub permission_id: String,
}

// ── Events (Bridge → Extension) ───────────────────────────────────────────────

/// A single streaming event emitted by the bridge over stdout.
/// The extension receives one or more of these per request, terminated by
/// `agent_finished` or `agent_error`.
#[derive(Debug, Serialize)]
pub struct RpcEvent {
    /// The request ID this event belongs to (matches the originating request).
    pub id: u64,
    /// The event name.
    pub event: RpcEventName,
    /// Event-specific payload.
    pub data: serde_json::Value,
}

/// All event names the bridge can emit. Mirrors `RpcEventName` in rpc.ts.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RpcEventName {
    /// A streamed text chunk from the LLM — append to previous chunks.
    TextDelta,
    /// A tool call has started.
    ToolStart,
    /// A tool call completed.
    ToolResult,
    /// Intermediate progress update from a long-running tool.
    ToolProgress,
    /// The agent is requesting user approval for a tool action.
    PermissionReq,
    /// Context window token count update.
    TokenUpdate,
    /// The agent loop completed successfully.
    AgentFinished,
    /// The agent loop terminated with an error.
    AgentError,
}

impl RpcEvent {
    /// Serialises the event to a JSON line and writes it to stdout.
    /// Each event is a single newline-terminated JSON object.
    pub fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            // Use writeln! to stdout — must be atomic per line so events
            // from concurrent tasks don't interleave mid-line.
            println!("{}", json);
        }
    }
}
