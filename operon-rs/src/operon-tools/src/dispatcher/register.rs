//! Tool registration methods for the dispatcher.
//!
//! Hey friend! This module provides easy registration methods for all built-in tool groups:
//! filesystem (8 tools), shell (1 tool), web (2 tools), todo (4 tools), memory (5 tools), and ask (1 tool).

use super::{Dispatcher, ToolEntry};
use operon_context_normalize_tools::{ToolCallId, ToolDefinition, ToolResult};
use operon_tools_core::ToolProgressEmitter;

impl Dispatcher {
    /// Registers a tool with the dispatcher.
    ///
    /// # Arguments
    /// - `definition`: The tool's canonical definition (from the tool crate's `definition()`).
    /// - `execute`: An async function `(ToolCallId, serde_json::Value, Option<ToolProgressEmitter>) -> Result<ToolResult, String>`.
    ///   Return `Err(reason)` if and only if the args failed to parse.
    ///   Runtime errors (e.g. file not found) must be returned as `Ok(ToolResult { is_error: true, ... })`.
    pub fn register<F, Fut>(&mut self, definition: ToolDefinition, execute: F)
    where
        F: Fn(ToolCallId, serde_json::Value, Option<ToolProgressEmitter>) -> Fut
            + Send
            + Sync
            + 'static,
        Fut: std::future::Future<Output = Result<ToolResult, String>> + Send + 'static,
    {
        let name = definition.name.clone();
        self.tools.insert(
            name,
            ToolEntry {
                definition,
                execute: Box::new(move |call_id, args, progress| {
                    Box::pin(execute(call_id, args, progress))
                }),
            },
        );
    }

    /// Registers all filesystem tools.
    ///
    /// Call this after `Dispatcher::new()` to make fs tools available.
    /// Registers: read, grep, glob, ls, edit, write, append, delete.
    pub fn register_fs_tools(&mut self) {
        self.register(
            operon_tools_fs_read::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_read::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_grep::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_grep::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_glob::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_glob::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_ls::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_ls::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_edit::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_edit::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_write::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_write::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_append::definition(),
            |call_id, args, progress| async move {
                operon_tools_fs_append::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_fs_delete::definition(),
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
        self.register(
            operon_tools_shell_bash::definition(),
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
        self.register(
            operon_tools_web_search::definition(),
            |call_id, args, progress| async move {
                operon_tools_web_search::execute_with_progress(call_id, args, progress)
                    .await
                    .map_err(|e| e.to_string())
            },
        );
        self.register(
            operon_tools_web_fetch::definition(),
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
    /// Definitions are registered here, and dispatch is routed directly in `dispatch_with_progress`.
    pub fn register_todo_tools(&mut self) {
        let defs = [
            operon_tools_todo_create::definition(),
            operon_tools_todo_list::definition(),
            operon_tools_todo_update::definition(),
            operon_tools_todo_delete::definition(),
        ];

        for def in defs {
            let name = def.name.clone();
            self.tools.insert(
                name.clone(),
                ToolEntry {
                    definition: def,
                    execute: Box::new(move |_call_id, _args, _progress| {
                        let n = name.clone();
                        Box::pin(async move {
                            Err(format!("todo tool '{}' should have been intercepted", n))
                        })
                    }),
                },
            );
        }
    }

    /// Registers all memory tools.
    ///
    /// Call this after `Dispatcher::new()` to make memory tools available.
    /// Memory tools require the MemoryStore, attached via `attach_memory_store()`.
    /// Definitions are registered here, and dispatch is routed directly in `dispatch_with_progress`.
    pub fn register_memory_tools(&mut self) {
        let defs = [
            operon_tools_memory_add::definition(),
            operon_tools_memory_edit::definition(),
            operon_tools_memory_delete::definition(),
            operon_tools_memory_retrieve::definition(),
            operon_tools_memory_search::definition(),
        ];

        for def in defs {
            let name = def.name.clone();
            self.tools.insert(
                name.clone(),
                ToolEntry {
                    definition: def,
                    execute: Box::new(move |_call_id, _args, _progress| {
                        let n = name.clone();
                        Box::pin(async move {
                            Err(format!("memory tool '{}' should have been intercepted", n))
                        })
                    }),
                },
            );
        }
    }

    /// Registers the `ask` tool.
    ///
    /// The `ask` tool is intercepted by the session runner before dispatch
    /// to pause and wait for user input on the command channel.
    /// This registration ensures its definition schema is exposed in the API tools payload.
    pub fn register_ask_tool(&mut self) {
        let name = "ask".to_string();
        self.tools.insert(
            name.clone(),
            ToolEntry {
                definition: operon_tools_ask::definition(),
                execute: Box::new(move |_call_id, _args, _progress| {
                    let n = name.clone();
                    Box::pin(async move {
                        Err(format!(
                            "'{}' must be intercepted by the runner, not dispatched",
                            n
                        ))
                    })
                }),
            },
        );
    }
}

