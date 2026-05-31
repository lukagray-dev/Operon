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
use operon_tools_core::{ReadLedger, ToolDispatchError};
use operon_tools_todo_create;
use operon_tools_todo_list;
use operon_tools_todo_update;
use operon_tools_todo_delete;
use operon_tools_web_search;
use operon_tools_web_fetch;

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
    /// Tracks paths read this session for read-before-write/edit enforcement.
    read_ledger: ReadLedger,
    /// In-memory todo list for the current agent session.
    todo_store: operon_tools_core::TodoStore,
}

impl Dispatcher {
    /// Creates an empty dispatcher with no tools registered.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            degraded: HashSet::new(),
            read_ledger: ReadLedger::new(),
            todo_store: operon_tools_core::TodoStore::new(),
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
        // Register all filesystem tools: read, grep, ls, edit, write, append, delete.
        // Each tool is registered with its tiered definition and async execute function.
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
        self.register(
            operon_tools_fs_append::definition(),
            |call_id, args| async move {
                operon_tools_fs_append::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_delete::definition(),
            |call_id, args| async move {
                operon_tools_fs_delete::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
    }

    /// Registers all shell tools.
    ///
    /// Call this after `Dispatcher::new()` to make shell tools available.
    /// Currently includes: bash.
    pub fn register_shell_tools(&mut self) {
        // Register all shell tools: bash.
        // Each tool is registered with its tiered definition and async execute function.
        self.register(
            operon_tools_shell_bash::definition(),
            |call_id, args| async move {
                operon_tools_shell_bash::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
    }

    /// Registers all web tools.
    ///
    /// Call this after `Dispatcher::new()` to make web tools available.
    /// Currently includes: web_search, web_fetch.
    pub fn register_web_tools(&mut self) {
        // Register all web tools: web_search, web_fetch.
        // Each tool is registered with its tiered definition and async execute function.
        self.register(
            operon_tools_web_search::definition(),
            |call_id, args| async move {
                operon_tools_web_search::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_web_fetch::definition(),
            |call_id, args| async move {
                operon_tools_web_fetch::execute(call_id, args)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
    }

    /// Registers all todo tools.
    ///
    /// Call this after `Dispatcher::new()` to make todo tools available.
    /// Todo tools are stateful — they require mutable access to todo_store.
    /// Definitions are registered here, but actual dispatch is handled directly
    /// in the dispatch() method with explicit routing.
    pub fn register_todo_tools(&mut self) {
        // Register todo tool definitions only — actual dispatch is handled directly
        // in dispatch() because todo tools require mutable access to todo_store.
        let defs = [
            operon_tools_todo_create::definition(),
            operon_tools_todo_list::definition(),
            operon_tools_todo_update::definition(),
            operon_tools_todo_delete::definition(),
        ];

        for def in defs {
            let name = def.name().to_string();
            self.tools.insert(name.clone(), ToolEntry {
                tiered: def,
                execute: Box::new(move |_call_id, _args| {
                    // This path is never reached — todo tools are intercepted in dispatch().
                    let n = name.clone();
                    Box::pin(async move {
                        Err(format!("todo tool '{}' should have been intercepted", n))
                    })
                }),
            });
        }
    }

    /// Clears the read ledger after context compaction.
    ///
    /// Must be called by the session runner whenever compaction fires.
    /// Compaction summarizes context — the model's mental model of file contents
    /// becomes stale. Clearing the ledger forces re-reads before any subsequent
    /// write or edit.
    pub fn notify_compaction(&mut self) {
        self.read_ledger.clear();
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
        let call_id = call.id.clone();

        // Todo tools: routed directly because they require mutable access to todo_store.
        match call.name.as_str() {
            "todo_create" => {
                return operon_tools_todo_create::execute(
                    call_id.clone(),
                    call.arguments,
                    &mut self.todo_store,
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_create", &e.to_string()));
            }
            "todo_list" => {
                return operon_tools_todo_list::execute(
                    call_id.clone(),
                    call.arguments,
                    &self.todo_store,
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_list", &e.to_string()));
            }
            "todo_update" => {
                return operon_tools_todo_update::execute(
                    call_id.clone(),
                    call.arguments,
                    &mut self.todo_store,
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_update", &e.to_string()));
            }
            "todo_delete" => {
                return operon_tools_todo_delete::execute(
                    call_id.clone(),
                    call.arguments,
                    &mut self.todo_store,
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_delete", &e.to_string()));
            }
            _ => {} // fall through to generic tool dispatch
        }

        // Look up the tool.
        let entry = match self.tools.get(&tool_name) {
            Some(e) => e,
            None => {
                return error_result(
                    call_id.clone(),
                    &tool_name,
                    &ToolDispatchError::UnknownTool { name: tool_name.clone() }.to_string(),
                );
            }
        };

        // Read-before-write/edit enforcement.
        // For `write` on existing files and all `edit` calls, the model must have
        // read the file at least once this session before modifying it.
        // New files (write to a path that doesn't exist) are exempt.
        if call.name == "write" || call.name == "edit" {
            if let Some(path_str) = call.arguments.get("path").and_then(|v| v.as_str()) {
                let path = std::path::Path::new(path_str);

                // For write: only enforce if the file already exists.
                // Creating a new file doesn't require a prior read.
                let requires_read = if call.name == "write" {
                    path.exists()
                } else {
                    // edit always requires a prior read — it modifies existing content.
                    true
                };

                if requires_read && !self.read_ledger.has_been_read(path) {
                    return error_result(
                        call_id.clone(),
                        &call.name,
                        &format!(
                            "read-before-{name} enforcement: '{path}' has not been read in this session. \
                             Use the read tool to read the file first, then retry.\n\
                             Note: if context compaction occurred recently, the ledger was reset — \
                             re-reading is required even if you read this file earlier.",
                            name = call.name,
                            path = path_str,
                        ),
                    );
                }
            }
            // If path arg is missing/malformed, fall through — the tool itself will
            // return a proper args error via the execute fn.
        }

        // Execute — the execute fn signals malformed args via Err(String).
        let tool_result = match (entry.execute)(call_id.clone(), call.arguments).await {
            Ok(result) => result,
            Err(reason) => {
                // Mark degraded so the next request sends the detailed description.
                self.degraded.insert(tool_name.clone());

                return error_result(
                    call_id.clone(),
                    &tool_name,
                    &ToolDispatchError::MalformedArgs {
                        tool: tool_name.clone(),
                        reason,
                    }
                    .to_string(),
                );
            }
        };

        // Record successful reads into the ledger.
        // A read is successful when is_error is false. We extract the paths that
        // were actually read from the tool result content.
        // Only fires for the `read` tool.
        if tool_name == "read" && !tool_result.is_error {
            record_read_paths(&mut self.read_ledger, &tool_result);
        }

        tool_result
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

    /// Returns a reference to the read ledger.
    ///
    /// Primarily for testing — allows asserting ledger state after tool calls.
    pub fn read_ledger(&self) -> &ReadLedger {
        &self.read_ledger
    }

    /// Returns a mutable reference to the todo store.
    ///
    /// Used by todo tool execute functions which receive the store as a parameter.
    pub fn todo_store_mut(&mut self) -> &mut operon_tools_core::TodoStore {
        &mut self.todo_store
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

/// Extracts successfully-read file paths from a `read` tool result and
/// records them in the ledger.
///
/// The `read` tool returns a JSON array of `FileReadResult` objects. Each has:
/// - `path: String` — the file path
/// - `success: bool` — true if the file was successfully read
/// - `error: Option<String>` — present and non-null if the file failed to read
///
/// Only paths with success=true (successfully read) are recorded.
/// If the content is not JSON or doesn't match the expected shape, this is a
/// no-op — we don't fail the dispatch over a ledger recording failure.
fn record_read_paths(ledger: &mut ReadLedger, result: &ToolResult) {
    // read tool returns ToolContent::Json with shape:
    // { "files": [ { "path": "...", "success": true, "error": null }, ... ] }
    let json = match &result.content {
        ToolContent::Json(v) => v,
        _ => return,
    };

    let files = match json.get("files").and_then(|f| f.as_array()) {
        Some(arr) => arr,
        None => return,
    };

    for file in files {
        // Only record if success is true (file was successfully read).
        let is_success = file
            .get("success")
            .and_then(|s| s.as_bool())
            .unwrap_or(false);

        if is_success {
            if let Some(path_str) = file.get("path").and_then(|p| p.as_str()) {
                ledger.record_read(std::path::Path::new(path_str));
            }
        }
    }
}
