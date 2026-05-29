//! Tool dispatcher — routes tool calls to implementations, manages tiered
//! descriptions per session, and handles malformed call recovery.
//!
//! # Session lifecycle
//!
//! Create one `Dispatcher` per agent session. It holds:
//! - The registry of all tools available in the session.
//! - A `HashSet` of tool names currently in "degraded" mode (detailed description active).
//!
//! On session end, drop the `Dispatcher`. A new session gets a fresh one with all tools
//! back in short-description mode.
//!
//! # Dispatch flow
//!
//! ```text
//! model emits ToolCall
//!   → dispatcher.dispatch(call) 
//!       → look up tool by name         [UnknownTool if missing]
//!       → parse args                   [MalformedArgs if fail → mark degraded → return error ToolResult]
//!       → execute tool                 [InternalError if runtime bug]
//!       → return ToolResult
//! ```
//!
//! # Getting tool definitions to send to the model
//!
//! ```text
//! dispatcher.definitions()
//!   → for each registered tool:
//!       if tool name is in degraded set → return detailed ToolDefinition
//!       else                            → return short ToolDefinition
//! ```

use std::collections::{HashMap, HashSet};
use operon_context_normalize_tools::{ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult};
use operon_tools_core::ToolDispatchError;

/// A registered tool: its tiered definition + its async execute function.
struct ToolEntry {
    tiered: operon_tools_core::TieredToolDefinition,
    /// Type-erased async execute fn.
    /// Takes (call_id, args_json) → ToolResult.
    /// Malformed args must be surfaced as Err(String) describing the parse failure.
    execute: Box<
        dyn Fn(
                ToolCallId,
                serde_json::Value,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<ToolResult, String>> + Send>,
            > + Send
            + Sync,
    >,
}

/// The tool dispatcher. One instance per agent session.
///
/// # Usage
///
/// ```rust
/// use operon_tools::dispatcher::Dispatcher;
///
/// let mut dispatcher = Dispatcher::new();
/// dispatcher.register_fs_tools();
///
/// // Get definitions to include in the next model request
/// let defs: Vec<_> = dispatcher.definitions().collect();
///
/// // After the model responds with a ToolCall:
/// // let result = dispatcher.dispatch(tool_call).await;
/// ```
pub struct Dispatcher {
    tools: HashMap<String, ToolEntry>,
    /// Tool names for which the model has made at least one malformed call
    /// in this session. These tools get the detailed description.
    degraded: HashSet<String>,
}

impl Dispatcher {
    /// Creates an empty dispatcher with no tools registered.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            degraded: HashSet::new(),
        }
    }

    /// Registers a tool with the dispatcher.
    ///
    /// # Arguments
    /// - `tiered`: The tool's tiered definition (from the tool crate's `definition()`).
    /// - `execute`: An async function `(ToolCallId, serde_json::Value) -> Result<ToolResult, String>`.
    ///   Return `Err(reason)` if and only if the args failed to parse.
    ///   Runtime errors (e.g. file not found) must be returned as `Ok(ToolResult { is_error: true, ... })`.
    pub fn register<F, Fut>(
        &mut self,
        tiered: operon_tools_core::TieredToolDefinition,
        execute: F,
    ) where
        F: Fn(ToolCallId, serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ToolResult, String>> + Send + 'static,
    {
        let name = tiered.name().to_string();
        self.tools.insert(
            name,
            ToolEntry {
                tiered,
                execute: Box::new(move |call_id, args| Box::pin(execute(call_id, args))),
            },
        );
    }

    /// Registers all filesystem tools.
    ///
    /// Call this after `Dispatcher::new()` to make fs tools available.
    /// As new tool groups are implemented, add analogous `register_*_tools` methods.
    pub fn register_fs_tools(&mut self) {
        self.register(
            operon_tools_fs_read::definition(),
            |call_id, args| async move {
                operon_tools_fs_read::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_grep::definition(),
            |call_id, args| async move {
                operon_tools_fs_grep::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_ls::definition(),
            |call_id, args| async move {
                operon_tools_fs_ls::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_edit::definition(),
            |call_id, args| async move {
                operon_tools_fs_edit::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_write::definition(),
            |call_id, args| async move {
                operon_tools_fs_write::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
    }

    /// Returns the tool definitions to include in the next model request.
    ///
    /// Each tool gets its `short` definition unless it is degraded (had a malformed
    /// call earlier in this session), in which case it gets `detailed`.
    pub fn definitions(&self) -> impl Iterator<Item = &ToolDefinition> {
        self.tools.values().map(|entry| {
            let degraded = self.degraded.contains(entry.tiered.name());
            entry.tiered.for_mode(degraded)
        })
    }

    /// Dispatches a tool call from the model to the correct implementation.
    ///
    /// # Returns
    /// Always returns a `ToolResult` — never propagates errors to the caller.
    /// Errors (unknown tool, malformed args, internal failures) are converted to
    /// `ToolResult { is_error: true, content: ToolContent::Text(reason) }` so the
    /// model can see what went wrong and recover.
    ///
    /// Side effect: if args parsing fails, the tool name is added to `self.degraded`
    /// so subsequent requests use the detailed description for that tool.
    pub async fn dispatch(&mut self, call: ToolCall) -> ToolResult {
        let tool_name = call.name.clone();

        // Look up the tool.
        let entry = match self.tools.get(&tool_name) {
            Some(e) => e,
            None => {
                return error_result(
                    call.id,
                    &tool_name,
                    &ToolDispatchError::UnknownTool { name: tool_name.clone() }.to_string(),
                );
            }
        };

        // Execute — the execute fn signals malformed args via Err(String).
        match (entry.execute)(call.id.clone(), call.arguments).await {
            Ok(result) => result,
            Err(reason) => {
                // Mark degraded so the next request sends the detailed description.
                self.degraded.insert(tool_name.clone());

                error_result(
                    call.id,
                    &tool_name,
                    &ToolDispatchError::MalformedArgs {
                        tool: tool_name.clone(),
                        reason,
                    }
                    .to_string(),
                )
            }
        }
    }

    /// Returns the set of tool names currently in degraded mode.
    ///
    /// Primarily useful for testing and diagnostics.
    pub fn degraded_tools(&self) -> &HashSet<String> {
        &self.degraded
    }

    /// Checks whether a specific tool is currently in degraded mode.
    pub fn is_degraded(&self, tool_name: &str) -> bool {
        self.degraded.contains(tool_name)
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Constructs an error `ToolResult` for dispatch-level failures.
fn error_result(call_id: ToolCallId, tool_name: &str, reason: &str) -> ToolResult {
    ToolResult {
        call_id,
        name: tool_name.to_string(),
        content: ToolContent::Text(reason.to_string()),
        is_error: true,
    }
}
