//! Tool dispatcher — routes tool calls to implementations, manages tiered
//! descriptions per session, handles malformed call recovery, and implements
//! lazy-loading of tool definitions to minimize token payload sizes.
//!
//! # Session lifecycle
//!
//! Create one `Dispatcher` per agent session. It holds:
//! - The registry of all tools available in the session.
//! - A `HashSet` of tool names currently in "degraded" mode (detailed description active).
//! - A `HashSet` of tool group names that have been loaded by the model in this session.
//!
//! On session end, drop the `Dispatcher`. A new session gets a fresh one with all tools
//! back in short-description mode and loaded tool groups reset.
//!
//! # Lazy Loading of Tools
//!
//! To avoid 413 "Payload Too Large" errors on models with smaller context limits,
//! the dispatcher exposes only the `load_tools` tool definition initially. The model
//! must call `load_tools { group: "group_name" }` to retrieve the definitions for a
//! specific tool group (like "fs"). Once loaded, those tools are unlocked and will
//! be returned in all subsequent `definitions()` calls for the rest of the session.
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
//!       if tool group is "core" OR tool group has been loaded:
//!           if tool name is in degraded set → return detailed ToolDefinition
//!           else                            → return short ToolDefinition
//!       else:
//!           skip tool (not exposed to model)
//! ```

use operon_context_normalize_tools::{
    ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult,
};
use operon_tools_core::{
    emit_tool_progress, ReadLedger, ToolDispatchError, ToolProgress, ToolProgressEmitter,
};
use operon_tools_load;
use operon_tools_ask;
use operon_tools_todo_create;
use operon_tools_todo_delete;
use operon_tools_todo_list;
use operon_tools_todo_update;
use operon_tools_web_fetch;
use operon_tools_web_search;
use std::collections::{HashMap, HashSet};

/// A registered tool: its tiered definition, group tag, and async execute function.
struct ToolEntry {
    tiered: operon_tools_core::TieredToolDefinition,
    /// The tool group this tool belongs to (e.g., "fs", "shell", "web", "todo", "core").
    /// Used by load_tools to filter definitions by group.
    group: &'static str,
    /// Type-erased async execute fn.
    /// Takes (call_id, args_json, progress) → ToolResult.
    /// Malformed args must be surfaced as Err(String) describing the parse failure.
    execute: Box<
        dyn Fn(
                ToolCallId,
                serde_json::Value,
                Option<ToolProgressEmitter>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<ToolResult, String>> + Send>,
            > + Send
            + Sync,
    >,
}

/// The result of a single tool dispatch, including metadata about side effects.
///
/// Returned by `Dispatcher::dispatch_with_progress()` instead of a bare
/// `ToolResult` so the session runner can observe dispatch-level side effects —
/// specifically, when a tool first enters degraded mode — and emit the
/// corresponding `SessionEvent` without the dispatcher needing to know about
/// the event bus.
///
/// # Why not return ToolResult directly?
///
/// The runner already processes the ToolResult for conversation history. Wrapping
/// it in `DispatchOutcome` adds zero overhead and avoids a dependency cycle:
/// `operon-tools` does not need to depend on `operon-events`.
pub struct DispatchOutcome {
    /// The tool result to feed back to the model and push into conversation history.
    pub result: ToolResult,
    /// `Some(name)` if this dispatch caused the named tool to be newly added to
    /// the degraded set (first malformed call this session). `None` if the tool
    /// was already degraded, or if dispatch succeeded / failed for other reasons.
    pub newly_degraded: Option<String>,
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
/// // let outcome = dispatcher.dispatch(tool_call).await;
/// // let result  = outcome.result;
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
    /// Tool groups that the AI model has requested and loaded in this session.
    /// By default, the AI only knows about basic/bootstrap tools (the "core" group).
    /// If the model needs to do file operations, it calls `load_tools` for the "fs" group,
    /// which unlocks those tools and stores "fs" here. Only tools belonging to groups
    /// in this set (plus the bootstrap "core" group) are sent to the provider.
    loaded_groups: HashSet<String>,
}

impl Dispatcher {
    /// Creates an empty dispatcher with no tools registered.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            degraded: HashSet::new(),
            read_ledger: ReadLedger::new(),
            todo_store: operon_tools_core::TodoStore::new(),
            // When starting a new session, the model hasn't loaded any tool groups yet.
            // So we start with an empty set. The model will request groups like "fs" on demand!
            loaded_groups: HashSet::new(),
        }
    }

    /// Registers a tool with the dispatcher.
    ///
    /// # Arguments
    /// - `tiered`: The tool's tiered definition (from the tool crate's `definition()`).
    /// - `group`: The tool group name (e.g., "fs", "shell", "web", "todo", "core").
    /// - `execute`: An async function `(ToolCallId, serde_json::Value, Option<ToolProgressEmitter>) -> Result<ToolResult, String>`.
    ///   Return `Err(reason)` if and only if the args failed to parse.
    ///   Runtime errors (e.g. file not found) must be returned as `Ok(ToolResult { is_error: true, ... })`.
    pub fn register<F, Fut>(
        &mut self,
        tiered: operon_tools_core::TieredToolDefinition,
        group: &'static str,
        execute: F,
    ) where
        F: Fn(ToolCallId, serde_json::Value, Option<ToolProgressEmitter>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: std::future::Future<Output = Result<ToolResult, String>> + Send + 'static,
    {
        let name = tiered.name().to_string();
        self.tools.insert(
            name,
            ToolEntry {
                tiered,
                group,
                execute: Box::new(move |call_id, args, progress| {
                    Box::pin(execute(call_id, args, progress))
                }),
            },
        );
    }

    /// Registers all filesystem tools.
    ///
    /// Call this after `Dispatcher::new()` to make fs tools available.
    /// As new tool groups are implemented, add analogous `register_*_tools` methods.
    pub fn register_fs_tools(&mut self) {
        // Register all filesystem tools: read, grep, ls, edit, write, append, delete.
        // Each tool is registered with its tiered definition, group tag, and async execute function.
        self.register(
            operon_tools_fs_read::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_read::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_grep::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_grep::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_ls::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_ls::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_edit::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_edit::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_write::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_write::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_append::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_append::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_delete::definition(),
            "fs",
            |call_id, args, progress| async move {
                operon_tools_fs_delete::execute_with_progress(call_id, args, progress)
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
        // Each tool is registered with its tiered definition, group tag, and async execute function.
        self.register(
            operon_tools_shell_bash::definition(),
            "shell",
            |call_id, args, progress| async move {
                operon_tools_shell_bash::execute_with_progress(call_id, args, progress)
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
        // Each tool is registered with its tiered definition, group tag, and async execute function.
        self.register(
            operon_tools_web_search::definition(),
            "web",
            |call_id, args, progress| async move {
                operon_tools_web_search::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_web_fetch::definition(),
            "web",
            |call_id, args, progress| async move {
                operon_tools_web_fetch::execute_with_progress(call_id, args, progress)
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
            self.tools.insert(
                name.clone(),
                ToolEntry {
                    tiered: def,
                    group: "todo",
                    execute: Box::new(move |_call_id, _args, _progress| {
                        // This path is never reached — todo tools are intercepted in dispatch().
                        let n = name.clone();
                        Box::pin(async move {
                            Err(format!("todo tool '{}' should have been intercepted", n))
                        })
                    }),
                },
            );
        }
    }

    /// Registers the load_tools tool.
    ///
    /// The load_tools tool is special: it's always available without loading,
    /// and it's intercepted directly in dispatch() before the generic tool lookup.
    /// This registration is for definition purposes only — the execute function
    /// is never called (dispatch intercepts it first).
    pub fn register_load_tool(&mut self) {
        let name = "load_tools".to_string();
        self.tools.insert(
            name.clone(),
            ToolEntry {
                tiered: operon_tools_load::definition(),
                group: "core", // load_tools is in the "core" group — not user-loadable
                execute: Box::new(move |_call_id, _args, _progress| {
                    // Never reached — load_tools is intercepted in dispatch().
                    let n = name.clone();
                    Box::pin(async move { Err(format!("'{}' should have been intercepted", n)) })
                }),
            },
        );
    }

    /// Registers the `ask` tool.
    ///
    /// The `ask` tool is in the "ask" group — loaded by the model via
    /// `load_tools { group: "ask" }`. It is NOT intercepted by the dispatcher;
    /// the session runner intercepts it before dispatch and handles the pause
    /// on the command channel. This registration is for definition purposes only.
    pub fn register_ask_tool(&mut self) {
        // Hey friend! We register the `ask` tool here. It belongs to the "ask" group.
        // It's not actually dispatched through here (the session runner intercepts it
        // beforehand to block and wait for the user response), but we register it so
        // that its definition schemas are known and can be loaded/lazy-loaded.
        let name = "ask".to_string();
        self.tools.insert(
            name.clone(),
            ToolEntry {
                tiered: operon_tools_ask::definition(),
                group: "ask",
                execute: Box::new(move |_call_id, _args, _progress| {
                    // Hey friend! This execute function is never reached because the session
                    // runner intercepts the tool call before dispatching. If it does run,
                    // we return an error indicating it should have been intercepted.
                    let n = name.clone();
                    Box::pin(async move {
                        Err(format!("'{}' must be intercepted by the runner, not dispatched", n))
                    })
                }),
            },
        );
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
        self.tools
            .values()
            .filter(|entry| {
                // Here, we restrict what tools are visible to the model to save context tokens.
                // 1. The "core" group (e.g. `load_tools`) is ALWAYS exposed so the model can
                //    actually load other groups when it needs them.
                // 2. Other tool groups (like "fs", "web", "shell") are only exposed if the model
                //    has explicitly loaded them using the `load_tools` tool in this session.
                entry.group == "core" || self.loaded_groups.contains(entry.group)
            })
            .map(|entry| {
                let degraded = self.degraded.contains(entry.tiered.name());
                entry.tiered.for_mode(degraded)
            })
    }

    /// Returns the short-tier `ToolDefinition` for every tool in the given group.
    ///
    /// Used by the `load_tools` executor to serve tool definitions on demand.
    /// Always returns the `short` tier — the `detailed` tier is only used for
    /// malformed call recovery and is never exposed via `load_tools`.
    pub fn definitions_for_group(
        &self,
        group: &str,
    ) -> Vec<&operon_context_normalize_tools::ToolDefinition> {
        self.tools
            .values()
            .filter(|entry| entry.group == group)
            .map(|entry| &entry.tiered.short)
            .collect()
    }

    /// Returns all unique group names currently registered, excluding internal groups.
    ///
    /// Used by `load_tools` when the model calls it with no group to list groups.
    /// Filters out the "core" group (internal tools like load_tools itself).
    pub fn registered_groups(&self) -> Vec<&str> {
        let mut groups: Vec<&str> = self
            .tools
            .values()
            .filter(|e| e.group != "core")
            .map(|e| e.group)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        groups.sort();
        groups
    }

    /// Dispatches a tool call from the model to the correct implementation.
    ///
    /// # Returns
    ///
    /// Always returns a [`DispatchOutcome`] — never propagates errors to the caller.
    /// Errors (unknown tool, malformed args, read-ledger violations) are converted to
    /// `ToolResult { is_error: true, content: ToolContent::Text(reason) }` so the
    /// model can see what went wrong and recover.
    ///
    /// The `newly_degraded` field in the outcome is `Some(name)` on the FIRST malformed
    /// call for a tool this session, allowing the runner to emit `SessionEvent::ToolDegraded`.
    /// Subsequent malformed calls for the same tool return `newly_degraded: None`.
    pub async fn dispatch(&mut self, call: ToolCall) -> ToolResult {
        self.dispatch_with_progress(call, None).await.result
    }

    /// Dispatches a tool call and forwards optional progress updates to the caller.
    pub async fn dispatch_with_progress(
        &mut self,
        call: ToolCall,
        progress: Option<ToolProgressEmitter>,
    ) -> DispatchOutcome {
        let tool_name = call.name.clone();
        let call_id = call.id.clone();

        // load_tools: intercepted directly because it needs access to dispatcher state.
        if call.name == "load_tools" {
            let group_arg = call
                .arguments
                .get("group")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let started_message = match &group_arg {
                Some(group) => format!("Loading tool group {}", group),
                None => "Listing available tool groups".to_string(),
            };
            emit_tool_progress(
                progress.as_ref(),
                ToolProgress::started(
                    call_id.clone(),
                    call.name.clone(),
                    group_arg.clone(),
                    started_message,
                ),
            );

            let result = if let Some(group) = &group_arg {
                let defs = self.definitions_for_group(group);
                operon_tools_load::execute_with_progress(
                    call_id.clone(),
                    group,
                    defs,
                    progress.clone(),
                )
            } else {
                let groups = self
                    .registered_groups()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>();
                operon_tools_load::execute_list_groups_with_progress(
                    call_id.clone(),
                    groups,
                    progress.clone(),
                )
            };

            // If the model successfully called `load_tools` for a specific group (result is not an error),
            // we record that group as "loaded". This means that starting from the NEXT turn, all tools in
            // this group will be appended to the model's available tools array so it can call them.
            if !result.is_error {
                if let Some(group) = &group_arg {
                    self.loaded_groups.insert(group.clone());
                }
            }

            emit_tool_progress(
                progress.as_ref(),
                if result.is_error {
                    ToolProgress::failed(
                        call_id.clone(),
                        call.name.clone(),
                        group_arg.clone(),
                        "load_tools failed",
                    )
                } else {
                    ToolProgress::completed(
                        call_id.clone(),
                        call.name.clone(),
                        group_arg.clone(),
                        "load_tools completed",
                    )
                },
            );

            return DispatchOutcome {
                result,
                newly_degraded: None,
            };
        }

        // Todo tools: routed directly because they require mutable access to todo_store.
        match call.name.as_str() {
            "todo_create" => {
                emit_tool_progress(
                    progress.as_ref(),
                    ToolProgress::started(
                        call_id.clone(),
                        call.name.clone(),
                        None,
                        "Creating todo item",
                    ),
                );
                let result = operon_tools_todo_create::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &mut self.todo_store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_create", &e.to_string()));
                emit_tool_progress(
                    progress.as_ref(),
                    if result.is_error {
                        ToolProgress::failed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_create failed",
                        )
                    } else {
                        ToolProgress::completed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_create completed",
                        )
                    },
                );
                return DispatchOutcome {
                    result,
                    newly_degraded: None,
                };
            }
            "todo_list" => {
                emit_tool_progress(
                    progress.as_ref(),
                    ToolProgress::started(
                        call_id.clone(),
                        call.name.clone(),
                        None,
                        "Listing todos",
                    ),
                );
                let result = operon_tools_todo_list::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &self.todo_store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_list", &e.to_string()));
                emit_tool_progress(
                    progress.as_ref(),
                    if result.is_error {
                        ToolProgress::failed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_list failed",
                        )
                    } else {
                        ToolProgress::completed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_list completed",
                        )
                    },
                );
                return DispatchOutcome {
                    result,
                    newly_degraded: None,
                };
            }
            "todo_update" => {
                emit_tool_progress(
                    progress.as_ref(),
                    ToolProgress::started(
                        call_id.clone(),
                        call.name.clone(),
                        None,
                        "Updating todo item",
                    ),
                );
                let result = operon_tools_todo_update::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &mut self.todo_store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_update", &e.to_string()));
                emit_tool_progress(
                    progress.as_ref(),
                    if result.is_error {
                        ToolProgress::failed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_update failed",
                        )
                    } else {
                        ToolProgress::completed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_update completed",
                        )
                    },
                );
                return DispatchOutcome {
                    result,
                    newly_degraded: None,
                };
            }
            "todo_delete" => {
                emit_tool_progress(
                    progress.as_ref(),
                    ToolProgress::started(
                        call_id.clone(),
                        call.name.clone(),
                        None,
                        "Deleting todo item",
                    ),
                );
                let result = operon_tools_todo_delete::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &mut self.todo_store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "todo_delete", &e.to_string()));
                emit_tool_progress(
                    progress.as_ref(),
                    if result.is_error {
                        ToolProgress::failed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_delete failed",
                        )
                    } else {
                        ToolProgress::completed(
                            call_id.clone(),
                            call.name.clone(),
                            None,
                            "todo_delete completed",
                        )
                    },
                );
                return DispatchOutcome {
                    result,
                    newly_degraded: None,
                };
            }
            _ => {} // fall through to generic tool dispatch
        }

        // Look up the tool.
        let entry = match self.tools.get(&tool_name) {
            Some(e) => e,
            None => {
                return DispatchOutcome {
                    result: error_result(
                        call_id.clone(),
                        &tool_name,
                        &ToolDispatchError::UnknownTool {
                            name: tool_name.clone(),
                        }
                        .to_string(),
                    ),
                    newly_degraded: None,
                };
            }
        };

        // Read-before-write/edit enforcement.
        // For `write` on existing files and all `edit` calls, the model must have
        // read the file at least once this session before modifying it.
        // New files (write to a path that doesn't exist) are exempt.
        if call.name == "write" || call.name == "edit" {
            if let Some(path_str) = call.arguments.get("path").and_then(|v| v.as_str()) {
                let path = std::path::Path::new(path_str);

                let requires_read = if call.name == "write" {
                    path.exists()
                } else {
                    true
                };

                if requires_read && !self.read_ledger.has_been_read(path) {
                    return DispatchOutcome {
                        result: error_result(
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
                        ),
                        newly_degraded: None,
                    };
                }
            }
        }

        // Execute — the execute fn signals malformed args via Err(String).
        emit_tool_progress(
            progress.as_ref(),
            ToolProgress::started(
                call_id.clone(),
                tool_name.clone(),
                None,
                format!("Dispatching {}", tool_name),
            ),
        );

        let tool_result =
            match (entry.execute)(call_id.clone(), call.arguments, progress.clone()).await {
                Ok(result) => result,
                Err(reason) => {
                    // Track whether this is the FIRST malformed call for this tool.
                    // The runner uses newly_degraded to emit SessionEvent::ToolDegraded.
                    let reason_text = reason.clone();
                    let was_degraded = self.degraded.contains(&tool_name);
                    self.degraded.insert(tool_name.clone());
                    let newly_degraded = if was_degraded {
                        None
                    } else {
                        Some(tool_name.clone())
                    };

                    emit_tool_progress(
                        progress.as_ref(),
                        ToolProgress::failed(
                            call_id.clone(),
                            tool_name.clone(),
                            None,
                            format!("{} malformed arguments: {}", tool_name, reason_text),
                        ),
                    );

                    return DispatchOutcome {
                        result: error_result(
                            call_id.clone(),
                            &tool_name,
                            &ToolDispatchError::MalformedArgs {
                                tool: tool_name.clone(),
                                reason,
                            }
                            .to_string(),
                        ),
                        newly_degraded,
                    };
                }
            };

        // Record successful reads into the ledger.
        if tool_name == "read" && !tool_result.is_error {
            record_read_paths(&mut self.read_ledger, &tool_result);
        }

        emit_tool_progress(
            progress.as_ref(),
            if tool_result.is_error {
                ToolProgress::failed(
                    call_id.clone(),
                    tool_name.clone(),
                    None,
                    format!("{} failed", tool_name),
                )
            } else {
                ToolProgress::completed(
                    call_id.clone(),
                    tool_name.clone(),
                    None,
                    format!("{} completed", tool_name),
                )
            },
        );

        DispatchOutcome {
            result: tool_result,
            newly_degraded: None,
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

    /// Returns a reference to the read ledger.
    ///
    /// Primarily for testing — allows asserting ledger state after tool calls.
    pub fn read_ledger(&self) -> &ReadLedger {
        &self.read_ledger
    }

    /// Returns the set of group names the model has explicitly loaded this session.
    /// This is helpful for serialization, testing, and debugging.
    pub fn loaded_groups(&self) -> &HashSet<String> {
        &self.loaded_groups
    }

    /// Mark a group as loaded — used when resuming a session that had previously
    /// loaded tool groups (inferred from conversation history).
    /// Since the model already knows the tool definitions from earlier in the chat,
    /// we ensure we continue sending those tool schemas in subsequent API requests.
    pub fn mark_group_loaded(&mut self, group: &str) {
        self.loaded_groups.insert(group.to_string());
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
