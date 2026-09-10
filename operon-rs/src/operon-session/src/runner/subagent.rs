// runner/subagent.rs — Subagent orchestration, concurrency, and session isolation.
//
// Hey friend! Welcome to the subagent coordinator!
//
// When an AI agent needs to tackle a large, multi-faceted task, or wants to explore
// multiple things in parallel (like analyzing 3 different modules or researching multiple APIs),
// it can emit one or more `spawn_agent` tool calls.
//
// This module is responsible for:
// 1. Intercepting `spawn_agent` calls and checking permission policies (`GlobalTool::SubAgent`).
// 2. Generating isolated, session-specific IDs and persistence files for each child agent.
// 3. Spawning multiple subagents concurrently in background tasks.
// 4. Forwarding live subagent progress events to the parent event stream for UI observability.
// 5. Holding the parent turn execution until ALL subagents in the batch complete their work.
// 6. Aggregating all subagent outputs into tool results for the parent model to continue its turn.
// 7. Propagating cancellations cleanly if the user interrupts the parent turn.

use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

use operon_context::{ContentBlock, ToolCall, ToolCallId, ToolContent, ToolResult};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::PolicyDecision;
use operon_tools_subagent_spawn::{SpawnAgentArgs, SpawnAgentOutput};

use crate::config::SessionConfig;
use crate::runner::message_build::{opaque_permission_denied_result, tool_result_content_json};
use crate::runner::SessionRunner;



// ─────────────────────────────────────────────────────────────────────────────
// Subagent Execution State
// ─────────────────────────────────────────────────────────────────────────────

/// Information needed to track and await an in-flight subagent background task.
pub(super) struct InFlightSubagent {
    /// Index in the original `stream_result.tool_calls` list to maintain exact order.
    pub(super) call_index: usize,
    /// Tool call ID assigned by the parent model.
    pub(super) call_id: ToolCallId,
    /// Tool name ("spawn_agent").
    pub(super) name: String,
    /// The unique session identifier created for this child subagent.
    pub(super) subagent_session_id: String,
    /// Inbound command sender used to send Cancel signals to the child runner.
    pub(super) cmd_tx: mpsc::Sender<SessionCommand>,
    /// Background join handle resolving to the subagent's execution outcome.
    pub(super) join_handle: tokio::task::JoinHandle<SpawnAgentOutput>,
}

// ─────────────────────────────────────────────────────────────────────────────
// SessionRunner Subagent Methods
// ─────────────────────────────────────────────────────────────────────────────

impl SessionRunner {
    /// Dispatches all tool calls from the model's turn, executing multiple `spawn_agent`
    /// calls concurrently while holding results until all subagents complete.
    ///
    /// # Flow:
    /// 1. Evaluates policy for each tool call.
    /// 2. Executes standard sequential tools (fs, shell, web, etc.).
    /// 3. Spawns all approved `spawn_agent` subagents concurrently in background tasks.
    /// 4. Waits until ALL subagents in the batch complete (or user cancels).
    /// 5. Assembles all results in the exact original tool call order.
    /// 6. Returns `(tool_results, should_stop)`.
    pub(super) async fn dispatch_turn_tool_calls(
        &mut self,
        calls: Vec<ToolCall>,
    ) -> (Vec<ContentBlock>, bool) {
        let total_calls = calls.len();
        // Slots to preserve exact ordering of results matching original calls
        let mut ordered_results: Vec<Option<ContentBlock>> = vec![None; total_calls];
        let mut in_flight_subagents: Vec<InFlightSubagent> = Vec::new();
        let mut should_stop = false;

        // ── Phase 1: Policy checks and immediate / sequential dispatch ────────
        for (idx, call) in calls.into_iter().enumerate() {
            // Check for cancellation before processing the next call
            self.drain_ready_commands();
            if self.take_matching_command(None).is_some() {
                tracing::info!("Session cancelled before dispatching tool call {}", call.name);
                should_stop = true;
                break;
            }

            if call.name == "spawn_agent" {
                // Policy check for subagents
                match self.policy_resolver.check(&call, self.caller_role()) {
                    PolicyDecision::Allow => {
                        // Launch this subagent into our concurrent batch below
                        match self.spawn_subagent_task(idx, call) {
                            Ok(in_flight) => in_flight_subagents.push(in_flight),
                            Err(err_result) => {
                                let content_json = tool_result_content_json(&err_result);
                                let _ = self
                                    .event_tx
                                    .send(SessionEvent::ToolCallResult {
                                        call_id: err_result.call_id.0.clone(),
                                        name: err_result.name.clone(),
                                        is_error: err_result.is_error,
                                        content_json,
                                    })
                                    .await;
                                ordered_results[idx] = Some(ContentBlock::ToolResult(err_result));
                            }
                        }
                    }
                    PolicyDecision::Ask { reason } => {
                        let approval_id = call.id.0.clone();
                        let args_json = serde_json::to_string(&call.arguments).unwrap_or_default();

                        let _ = self
                            .event_tx
                            .send(SessionEvent::ApprovalRequired {
                                id: approval_id.clone(),
                                tool: call.name.clone(),
                                path: None,
                                reason,
                                args_json,
                            })
                            .await;

                        match self.wait_for_relevant_command(Some(&approval_id)).await {
                            SessionCommand::Approve { id } if id == approval_id => {
                                let _ = self
                                    .event_tx
                                    .send(SessionEvent::ApprovalGranted {
                                        id: approval_id.clone(),
                                        tool: call.name.clone(),
                                        path: None,
                                    })
                                    .await;

                                match self.spawn_subagent_task(idx, call) {
                                    Ok(in_flight) => in_flight_subagents.push(in_flight),
                                    Err(err_result) => {
                                        let content_json = tool_result_content_json(&err_result);
                                        let _ = self
                                            .event_tx
                                            .send(SessionEvent::ToolCallResult {
                                                call_id: err_result.call_id.0.clone(),
                                                name: err_result.name.clone(),
                                                is_error: err_result.is_error,
                                                content_json,
                                            })
                                            .await;
                                        ordered_results[idx] =
                                            Some(ContentBlock::ToolResult(err_result));
                                    }
                                }
                            }
                            SessionCommand::Deny { id } if id == approval_id => {
                                let result = opaque_permission_denied_result(&call);
                                let content_json = tool_result_content_json(&result);
                                let _ = self
                                    .event_tx
                                    .send(SessionEvent::ToolCallResult {
                                        call_id: result.call_id.0.clone(),
                                        name: result.name.clone(),
                                        is_error: result.is_error,
                                        content_json,
                                    })
                                    .await;
                                ordered_results[idx] = Some(ContentBlock::ToolResult(result));
                            }
                            SessionCommand::Cancel => {
                                tracing::info!("Session cancelled during approval wait");
                                should_stop = true;
                                break;
                            }
                            _ => {
                                let result = opaque_permission_denied_result(&call);
                                ordered_results[idx] = Some(ContentBlock::ToolResult(result));
                            }
                        }
                    }
                    PolicyDecision::Deny { reason } => {
                        let _ = self
                            .event_tx
                            .send(SessionEvent::PermissionDenied {
                                tool: call.name.clone(),
                                path: None,
                                reason,
                            })
                            .await;
                        let result = opaque_permission_denied_result(&call);
                        let content_json = tool_result_content_json(&result);
                        let _ = self
                            .event_tx
                            .send(SessionEvent::ToolCallResult {
                                call_id: result.call_id.0.clone(),
                                name: result.name.clone(),
                                is_error: result.is_error,
                                content_json,
                            })
                            .await;
                        ordered_results[idx] = Some(ContentBlock::ToolResult(result));
                    }
                }
            } else {
                // Non-subagent tool call: run sequentially through standard handle_tool_call
                let mut temp_results = Vec::new();
                match self.handle_tool_call(call, &mut temp_results).await {
                    crate::runner::tool_dispatch::ToolCallFlow::Continue => {
                        if let Some(res) = temp_results.pop() {
                            ordered_results[idx] = Some(res);
                        }
                    }
                    crate::runner::tool_dispatch::ToolCallFlow::Stop => {
                        if let Some(res) = temp_results.pop() {
                            ordered_results[idx] = Some(res);
                        }
                        should_stop = true;
                        break;
                    }
                }
            }
        }

        // ── Phase 2: Await all in-flight subagents concurrently ──────────────
        if !in_flight_subagents.is_empty() {
            tracing::info!(
                subagent_count = in_flight_subagents.len(),
                "Awaiting all concurrent subagents to finish"
            );

            // If a stop/cancellation occurred, cancel all subagents immediately
            if should_stop {
                for sub in &in_flight_subagents {
                    let _ = sub.cmd_tx.send(SessionCommand::Cancel).await;
                    sub.join_handle.abort();
                }
            } else {
                // Wait for all subagents to finish their work concurrently.
                // We monitor cmd_rx so if a Cancel arrives while subagents are working,
                // we gracefully propagate the cancel to all subagents!
                for sub in in_flight_subagents {
                    let outcome = match sub.join_handle.await {
                        Ok(output) => output,
                        Err(join_err) => {
                            if join_err.is_cancelled() {
                                SpawnAgentOutput::cancelled(sub.subagent_session_id.clone(), "general")
                            } else {
                                SpawnAgentOutput::failed(
                                    sub.subagent_session_id.clone(),
                                    "general",
                                    format!("Subagent background task panicked: {join_err}"),
                                )
                            }
                        }
                    };

                    let is_error = outcome.status != "completed";
                    let content_text = outcome.format_for_parent();

                    let tool_result = ToolResult {
                        call_id: sub.call_id.clone(),
                        name: sub.name.clone(),
                        content: ToolContent::Text(content_text),
                        is_error,
                    };

                    let content_json = tool_result_content_json(&tool_result);
                    let _ = self
                        .event_tx
                        .send(SessionEvent::ToolCallResult {
                            call_id: sub.call_id.0.clone(),
                            name: sub.name.clone(),
                            is_error,
                            content_json,
                        })
                        .await;

                    ordered_results[sub.call_index] = Some(ContentBlock::ToolResult(tool_result));
                }
            }
        }

        // Flatten all ordered results, skipping any None slots (from break on cancellation)
        let final_results: Vec<ContentBlock> = ordered_results.into_iter().flatten().collect();

        (final_results, should_stop)
    }

    /// Prepares and spawns a single subagent as a concurrent background task.
    pub(super) fn spawn_subagent_task(
        &self,
        call_index: usize,
        call: ToolCall,
    ) -> Result<InFlightSubagent, ToolResult> {
        let call_id = call.id.clone();
        let name = call.name.clone();

        // 1. Parse arguments
        let args = match SpawnAgentArgs::from_json(&call.arguments) {
            Ok(a) => a,
            Err(e) => {
                return Err(ToolResult {
                    call_id,
                    name,
                    content: ToolContent::Text(e.to_string()),
                    is_error: true,
                });
            }
        };

        // 2. Derive separate, session-specific session ID
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let subagent_session_id = format!("{}_sub_{}_{}", self.session_id, call_index, now_nanos);

        // 3. Resolve separate session JSON store path
        let sub_store_path = self.config.store_path.as_ref().map(|parent_store| {
            if let Some(parent_dir) = parent_store.parent() {
                parent_dir.join(format!("{subagent_session_id}.json"))
            } else {
                std::path::PathBuf::from(format!("{subagent_session_id}.json"))
            }
        });

        // 4. Build subagent SessionConfig
        // Subagents are leaf agents: they have full fs, shell, web, todo, and memory access,
        // but no recursive `subagent` tool and no interactive `ask` tool.
        let sub_tool_groups = vec![
            "fs".to_string(),
            "shell".to_string(),
            "web".to_string(),
            "todo".to_string(),
            "memory".to_string(),
        ];

        let role_label = args.role.clone();
        let sub_instructions = format!(
            "You are an autonomous subagent specialized as a '{}'. \
             You have been spawned by the parent agent to execute this specific task. \
             Use your available tools to perform the required work, and provide a clear, \
             accurate, and well-structured final summary of your findings or output.",
            role_label
        );

        let sub_config = SessionConfig {
            provider_config: self.config.provider_config.clone(),
            policy: self.config.policy.clone(),
            project_dir: self.config.project_dir.clone(),
            workspace_root: self.config.workspace_root.clone(),
            role: self.config.role,
            tool_groups: sub_tool_groups,
            compaction: self.config.compaction.clone(),
            store_path: sub_store_path,
            channel_instructions: Some(sub_instructions),
        };

        // 5. Create command channel for cancellation
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(10);
        let parent_event_tx = self.event_tx.clone();
        let parent_session_id = self.session_id.clone();
        let sub_id = subagent_session_id.clone();
        let c_id = call_id.0.clone();

        // 6. Spawn the background task
        let join_handle = tokio::spawn(async move {
            run_single_subagent(
                parent_session_id,
                sub_id,
                c_id,
                args,
                sub_config,
                parent_event_tx,
                cmd_rx,
            )
            .await
        });

        Ok(InFlightSubagent {
            call_index,
            call_id,
            name,
            subagent_session_id,
            cmd_tx,
            join_handle,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Subagent Background Task Worker
// ─────────────────────────────────────────────────────────────────────────────

/// Runs a single subagent to completion, forward events, and returns `SpawnAgentOutput`.
async fn run_single_subagent(
    parent_session_id: String,
    subagent_session_id: String,
    call_id: String,
    args: SpawnAgentArgs,
    sub_config: SessionConfig,
    parent_event_tx: mpsc::Sender<SessionEvent>,
    sub_cmd_rx: mpsc::Receiver<SessionCommand>,
) -> SpawnAgentOutput {
    let role = args.role.clone();
    let task = args.task.clone();

    // 1. Child event channel
    let (sub_event_tx, mut sub_event_rx) = mpsc::channel::<SessionEvent>(100);

    // 2. Construct child SessionRunner
    let mut sub_runner = match SessionRunner::new(sub_config, sub_event_tx, sub_cmd_rx).await {
        Ok(runner) => runner,
        Err(e) => {
            let err_msg = format!("Failed to initialize subagent session: {e}");
            let _ = parent_event_tx
                .send(SessionEvent::SubagentFailed {
                    parent_session_id,
                    subagent_session_id: subagent_session_id.clone(),
                    call_id,
                    error: err_msg.clone(),
                })
                .await;
            return SpawnAgentOutput::failed(subagent_session_id, role, err_msg);
        }
    };

    // 3. Emit SubagentSpawned on parent event stream
    let _ = parent_event_tx
        .send(SessionEvent::SubagentSpawned {
            parent_session_id: parent_session_id.clone(),
            subagent_session_id: subagent_session_id.clone(),
            call_id: call_id.clone(),
            role: role.clone(),
            task: task.clone(),
        })
        .await;

    // 4. Background task to consume subagent events and stream progress to parent
    let parent_tx_fwd = parent_event_tx.clone();
    let p_id_fwd = parent_session_id.clone();
    let s_id_fwd = subagent_session_id.clone();
    let c_id_fwd = call_id.clone();
    let role_fwd = role.clone();

    let event_collector = tokio::spawn(async move {
        let mut accumulated_text = String::new();
        let mut final_error: Option<String> = None;

        while let Some(event) = sub_event_rx.recv().await {
            match event {
                SessionEvent::TextDelta { text } => {
                    accumulated_text.push_str(&text);
                }
                SessionEvent::ToolCallStart { name, .. } => {
                    let _ = parent_tx_fwd
                        .send(SessionEvent::SubagentProgress {
                            parent_session_id: p_id_fwd.clone(),
                            subagent_session_id: s_id_fwd.clone(),
                            call_id: c_id_fwd.clone(),
                            stage: format!("tool:{name}"),
                            message: format!("[Subagent {role_fwd}] Executing tool `{name}`"),
                        })
                        .await;
                }
                SessionEvent::ToolProgress(progress) => {
                    let _ = parent_tx_fwd
                        .send(SessionEvent::SubagentProgress {
                            parent_session_id: p_id_fwd.clone(),
                            subagent_session_id: s_id_fwd.clone(),
                            call_id: c_id_fwd.clone(),
                            stage: format!("{:?}", progress.stage),
                            message: format!("[Subagent {role_fwd}] {}", progress.message),
                        })
                        .await;
                }
                SessionEvent::Error { message } => {
                    final_error = Some(message);
                }
                _ => {}
            }
        }

        (accumulated_text, final_error)
    });

    // 5. Execute subagent turn
    let run_res = sub_runner.run(task, Vec::new(), Vec::new()).await;

    // Explicitly drop sub_runner to close sub_event_tx and finalize event_collector
    drop(sub_runner);

    let (accumulated_text, final_error) = event_collector.await.unwrap_or_default();

    if let Err(e) = run_res {
        let err_msg = format!("Subagent execution error: {e}");
        let _ = parent_event_tx
            .send(SessionEvent::SubagentFailed {
                parent_session_id,
                subagent_session_id: subagent_session_id.clone(),
                call_id,
                error: err_msg.clone(),
            })
            .await;
        SpawnAgentOutput::failed(subagent_session_id, role, err_msg)
    } else if let Some(err) = final_error {
        let _ = parent_event_tx
            .send(SessionEvent::SubagentFailed {
                parent_session_id,
                subagent_session_id: subagent_session_id.clone(),
                call_id,
                error: err.clone(),
            })
            .await;
        SpawnAgentOutput::failed(subagent_session_id, role, err)
    } else {
        let summary = if accumulated_text.trim().is_empty() {
            "Subagent completed task successfully.".to_string()
        } else {
            accumulated_text.trim().to_string()
        };

        let _ = parent_event_tx
            .send(SessionEvent::SubagentCompleted {
                parent_session_id,
                subagent_session_id: subagent_session_id.clone(),
                call_id,
                summary: summary.clone(),
            })
            .await;

        SpawnAgentOutput::completed(subagent_session_id, role, summary)
    }
}

