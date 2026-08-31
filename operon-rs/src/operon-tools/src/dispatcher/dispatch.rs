//! Tool dispatch pipeline and execution logic.
//!
//! Hey friend! This module contains the runtime execution routing for all model tool calls,
//! including special stateful tool interceptions (todo and memory) and read-before-write checks.

use super::ledger::record_read_paths;
use super::{error_result, DispatchOutcome, Dispatcher};
use operon_context_normalize_tools::{ToolCall, ToolResult};
use operon_tools_core::{emit_tool_progress, ToolDispatchError, ToolProgress, ToolProgressEmitter};

impl Dispatcher {
    /// Dispatches a tool call from the model to the correct implementation.
    ///
    /// # Returns
    ///
    /// Always returns a [`ToolResult`] — never propagates errors to the caller.
    /// Errors (unknown tool, malformed args, read-ledger violations) are converted to
    /// `ToolResult { is_error: true, content: ToolContent::Text(reason) }` so the
    /// model can see what went wrong and recover.
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

        // ── 1. Stateful Todo Tools Interception ───────────────────────────────
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
                return DispatchOutcome { result };
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
                return DispatchOutcome { result };
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
                return DispatchOutcome { result };
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
                return DispatchOutcome { result };
            }
            _ => {} // fall through to memory or generic tool dispatch
        }

        // ── 2. Stateful Memory Tools Interception ─────────────────────────────
        if matches!(
            call.name.as_str(),
            "memory_add" | "memory_edit" | "memory_delete" | "memory_retrieve" | "memory_search"
        ) {
            let store = match &self.memory_store {
                Some(s) => s.clone(),
                None => {
                    return DispatchOutcome {
                        result: error_result(
                            call_id.clone(),
                            &call.name,
                            "memory store not available (attach_memory_store was not called)",
                        ),
                    };
                }
            };

            let tool_name_str = call.name.as_str();
            let label = match tool_name_str {
                "memory_add" => "Storing memory",
                "memory_edit" => "Updating memory",
                "memory_delete" => "Deleting memory",
                "memory_retrieve" => "Retrieving memories",
                "memory_search" => "Searching memories",
                _ => "Processing memory tool",
            };

            emit_tool_progress(
                progress.as_ref(),
                ToolProgress::started(call_id.clone(), call.name.clone(), None, label),
            );

            let result = match call.name.as_str() {
                "memory_add" => operon_tools_memory_add::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "memory_add", &e.to_string())),

                "memory_edit" => operon_tools_memory_edit::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "memory_edit", &e.to_string())),

                "memory_delete" => operon_tools_memory_delete::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "memory_delete", &e.to_string())),

                "memory_retrieve" => operon_tools_memory_retrieve::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| {
                    error_result(call_id.clone(), "memory_retrieve", &e.to_string())
                }),

                "memory_search" => operon_tools_memory_search::execute_with_progress(
                    call_id.clone(),
                    call.arguments,
                    &store,
                    progress.clone(),
                )
                .await
                .unwrap_or_else(|e| error_result(call_id.clone(), "memory_search", &e.to_string())),

                _ => unreachable!("matches! guard above ensures only memory tools reach here"),
            };

            emit_tool_progress(
                progress.as_ref(),
                if result.is_error {
                    ToolProgress::failed(
                        call_id.clone(),
                        call.name.clone(),
                        None,
                        format!("{} failed", call.name),
                    )
                } else {
                    ToolProgress::completed(
                        call_id.clone(),
                        call.name.clone(),
                        None,
                        format!("{} completed", call.name),
                    )
                },
            );

            return DispatchOutcome { result };
        }

        // ── 3. Tool Lookup ───────────────────────────────────────────────────
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
                };
            }
        };

        // ── 4. Read-before-write/edit Enforcement ────────────────────────────
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
                    };
                }
            }
        }

        // ── 5. Tool Execution ────────────────────────────────────────────────
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
                    let reason_text = reason.clone();
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
                    };
                }
            };

        // ── 6. Record Successful Reads into Ledger ───────────────────────────
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
        }
    }
}

