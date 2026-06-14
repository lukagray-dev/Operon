// policy.rs — Policy configuration helpers and runtime role checks for SessionRunner.
//
// Hey friend! This file houses the filesystem path extraction helper and opaque error
// constructors for tool calls checked against the policy engine, as well as the
// caller_role method on SessionRunner.

use crate::runner::SessionRunner;
use operon_context::{Role, ToolCall, ToolContent, ToolResult};
use operon_policy::CallerRole;

impl SessionRunner {
    /// Convert the session runtime role into the policy crate role.
    pub(crate) fn caller_role(&self) -> CallerRole {
        match self.config.role {
            Role::Owner => CallerRole::Owner,
            Role::External => CallerRole::External,
        }
    }
}

/// Build the policy-facing path string for a tool call, if the tool uses one.
///
/// This helper extracts a representative filesystem path from the tool call's arguments.
/// The extracted path is used by the policy resolver to check whether the caller has
/// permission to access or operate on that specific path.
pub fn policy_path_for_call(call: &ToolCall) -> Option<String> {
    match call.name.as_str() {
        // The "read" tool takes a whitespace-delimited string in its "paths" attr
        // (e.g. paths="C:\\file1.txt C:\\file2.txt:40-90").
        // We extract the first path entry as the representative path for policy checks.
        "read" => call
            .arguments
            .get("paths")
            .and_then(|v| v.as_str())
            .and_then(|s| s.split_whitespace().next())
            .map(|first| first.trim().to_string()),

        // The "bash" tool executes commands within a specific directory. We extract the "path"
        // argument to check whether shell execution is permitted in that directory.
        "bash" => call
            .arguments
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),

        // Filesystem modification or lookup tools (write, edit, append, ls, delete, grep) operate on a single path.
        // We look for a singular "path" argument (e.g. path: "dir/file.txt") and extract its value as a string.
        "write" | "edit" | "append" | "ls" | "delete" | "grep" => call
            .arguments
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),

        // Any other tool is considered global or doesn't target specific filesystem paths,
        // so we return None and bypass directory-specific policy check gates.
        _ => None,
    }
}

/// Construct the opaque error result we return to the model when policy blocks a call.
pub fn opaque_permission_denied_result(call: &ToolCall) -> ToolResult {
    ToolResult {
        call_id: call.id.clone(),
        name: call.name.clone(),
        content: ToolContent::Text("Tool not available.".to_string()),
        is_error: true,
        // read_paths is None — this is a denied call, no files were read.
        read_paths: None,
    }
}
