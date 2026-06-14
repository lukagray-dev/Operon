// runner.rs — The Operon agent loop runner.
//
// Hey friend! This file houses the SessionRunner struct, which is the main orchestrator
// of the session crate. It implements the core agentic loop (run()) which calls out to the
// session submodules for specialized tasks such as initialization, ask interception, policy
// verification and tool dispatch, and event emitting.

use std::collections::VecDeque;

use reqwest::Client;
use tokio::sync::mpsc;

use operon_config::PolicyConfig;
use operon_context::{
    sanitize, ContentBlock, ConversationMessage, MessageRole, SessionTokenState, SnapshotBuilder,
    TokenBudget, ToolCallId, ToolContent,
};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::PolicyResolver;
use operon_providers::Provider;
use operon_tools::dispatcher::Dispatcher;

use crate::config::SessionConfig;
use crate::error::SessionError;
use crate::lifecycle::LifecycleState;
use crate::request::build_request;
use crate::store::SessionStore;
use crate::session;
use crate::http::send_streaming;

// Re-exports required by the test runner (runner_tests.rs) which imports via `use super::*`.
#[cfg(test)]
pub(crate) use crate::http::StreamResult;
#[cfg(test)]
pub(crate) use session::commands::command_matches;
#[cfg(test)]
pub(crate) use session::policy::{opaque_permission_denied_result, policy_path_for_call};
#[cfg(test)]
pub(crate) use session::events::{build_assistant_message, context_usage_event, tool_result_content_json};
#[cfg(test)]
pub(crate) use operon_context::Role;

/// The Operon agent loop — owns all session state and drives the agentic cycle.
///
/// # Thread safety
///
/// `SessionRunner` is `Send` but not `Sync` (held in a single async task).
/// Wrap in `Arc<Mutex<...>>` only if you need cross-task access.
///
/// # Lifecycle
///
/// 1. `SessionRunner::new(config, event_tx, cmd_rx)` — create and initialize.
/// 2. `runner.run(user_message)` — enter the agent loop.
/// 3. Events flow over `event_tx` until `SessionEvent::Done` or `SessionEvent::Error`.
/// 4. Send `SessionCommand::Cancel` on `cmd_tx` to stop the loop cleanly.
pub struct SessionRunner {
    /// Unique identifier for this session (hex nanoseconds).
    pub(crate) session_id: String,
    /// Runtime configuration (provider, model, tool groups, policy, etc.).
    pub(crate) config: SessionConfig,
    /// The full conversation history including all turns in this session.
    pub(crate) messages: Vec<ConversationMessage>,
    /// Tool dispatcher — routes tool calls to implementations.
    pub(crate) dispatcher: Dispatcher,
    /// Snapshot builder — generates the system prompt block per turn.
    pub(crate) snapshot_builder: SnapshotBuilder,
    /// Per-session exact token state (updated from API usage blocks).
    pub(crate) token_state: SessionTokenState,
    /// Immutable token budget for the model's context window.
    pub(crate) token_budget: TokenBudget,
    /// Current lifecycle state machine.
    pub(crate) lifecycle: LifecycleState,
    /// Shared HTTP client (clone-able for the compaction client).
    pub(crate) http_client: Client,
    /// Outbound event channel — UI/tests receive from the other end.
    pub(crate) event_tx: mpsc::Sender<SessionEvent>,
    /// Inbound command channel — UI sends Cancel/Approve/Deny into the loop.
    pub(crate) cmd_rx: mpsc::Receiver<SessionCommand>,
    /// Policy engine for permission checks before tool dispatch.
    pub(crate) policy_resolver: PolicyResolver,
    /// Buffered inbound commands that arrived before the runner was waiting.
    pub(crate) pending_commands: VecDeque<SessionCommand>,
    /// Optional SQLite store for turn persistence.
    pub(crate) store: Option<SessionStore>,
    /// 0-based index of the next turn to execute.
    pub(crate) turn_index: usize,
}

impl SessionRunner {
    /// Create a new session runner. Does not start the loop.
    ///
    /// Delegates construction to the session::init submodule.
    pub async fn new(
        config: SessionConfig,
        event_tx: mpsc::Sender<SessionEvent>,
        cmd_rx: mpsc::Receiver<SessionCommand>,
    ) -> Result<Self, SessionError> {
        session::init::new_runner(config, event_tx, cmd_rx).await
    }

    /// Load conversation history, turn index, and last token count to resume a session.
    pub fn set_history(
        &mut self,
        messages: Vec<ConversationMessage>,
        turn_index: usize,
        last_token_count: Option<usize>,
    ) {
        // When resuming an existing session, we need to inspect the conversation history
        // to find any tool groups (like "fs") that the AI model previously requested to load.
        // Restoring this state ensures we include those tools in the `tools` array of the very first
        // API request of this resumed session, so the AI model can continue using them immediately.
        for msg in &messages {
            for block in &msg.content {
                // We are looking for tool results in the conversation history...
                if let ContentBlock::ToolResult(result) = block {
                    // ...specifically, successful executions of the "load_tools" tool.
                    if result.name == "load_tools" && !result.is_error {
                        // The output content of load_tools is now returned as a plain-text description string.
                        // Format: "Loaded <count> tool(s) from group '<group_name>':"
                        let ToolContent::Text(ref text) = result.content;
                        if let Some(start_idx) = text.find("from group '") {
                            let start = start_idx + "from group '".len();
                            if let Some(end_idx) = text[start..].find('\'') {
                                let group = &text[start..start + end_idx];
                                self.dispatcher.mark_group_loaded(group);
                            }
                        }
                    }
                }
            }
        }

        self.messages = messages;
        self.turn_index = turn_index;
        if let Some(tokens) = last_token_count {
            self.token_state
                .apply_estimate(tokens, operon_context::EstimationTier::Exact);
        }
    }

    /// Run one user turn through the agent loop.
    pub async fn run(&mut self, user_message: String) -> Result<(), SessionError> {
        // Guard: only Idle and Paused sessions may enter the loop.
        if !self.lifecycle.can_run() {
            return Err(SessionError::InvalidState {
                state: format!("{:?}", self.lifecycle),
            });
        }
        self.lifecycle = LifecycleState::Running;

        // Push the user's message into the conversation history.
        self.messages
            .push(ConversationMessage::user(vec![ContentBlock::Text(
                user_message,
            )]));

        // The agent loop — continues until the model returns no tool calls.
        loop {
            // ── 1. Compaction check ──────────────────────────────────────────
            if self
                .token_budget
                .should_compact(self.token_state.current_context_tokens)
            {
                // Notify the UI that compaction is about to run so it can show a spinner.
                let _ = self
                    .event_tx
                    .send(SessionEvent::CompactionStarted {
                        tokens_before: self.token_state.current_context_tokens,
                    })
                    .await;

                match self.run_compaction().await {
                    Ok(()) => {}
                    Err(SessionError::Compaction(
                        operon_context::CompactionError::ThresholdNotReached,
                    )) => {
                        tracing::warn!("Compaction triggered but threshold not reached — skipping");
                    }
                    Err(SessionError::Compaction(
                        operon_context::CompactionError::InsufficientHistory,
                    )) => {
                        let _ = self
                            .event_tx
                            .send(SessionEvent::Warning {
                                message: "Context compaction skipped: insufficient history"
                                    .to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        // If compaction encounters a fatal error, emit PreTurnFailed,
                        // transition session lifecycle state to Failed, and return the error.
                        let _ = self
                            .event_tx
                            .send(SessionEvent::PreTurnFailed {
                                turn_index: self.turn_index,
                                step: operon_events::PreTurnStep::Compaction,
                                reason: e.to_string(),
                            })
                            .await;
                        self.lifecycle = LifecycleState::Failed;
                        return Err(e);
                    }
                }
            }

            // ── 2. Build snapshot ────────────────────────────────────────────
            // We build a filesystem/project snapshot that captures the current workspace file tree
            // and files status, which the AI model uses to understand project context.
            // If snapshotting fails, we emit a PreTurnFailed event so the UI can notify the user,
            // transition the lifecycle state to Failed, and return the error.
            let snapshot = match self.snapshot_builder.build() {
                Ok(s) => s,
                Err(e) => {
                    let _ = self
                        .event_tx
                        .send(SessionEvent::PreTurnFailed {
                            turn_index: self.turn_index,
                            step: operon_events::PreTurnStep::Snapshot,
                            reason: e.to_string(),
                        })
                        .await;
                    self.lifecycle = LifecycleState::Failed;
                    return Err(e.into());
                }
            };

            // ── 2b. Sanitize conversation messages ───────────────────────────
            // We sanitize the conversation messages to strip out any invalid blocks,
            // inject the system prompt snapshot, and prepare the history for the provider.
            // If sanitization fails, we emit PreTurnFailed, set session state to Failed, and exit.
            let clean_messages = match sanitize(self.messages.clone(), &snapshot, self.config.role)
            {
                Ok(m) => m,
                Err(e) => {
                    let _ = self
                        .event_tx
                        .send(SessionEvent::PreTurnFailed {
                            turn_index: self.turn_index,
                            step: operon_events::PreTurnStep::Sanitizer,
                            reason: e.to_string(),
                        })
                        .await;
                    self.lifecycle = LifecycleState::Failed;
                    return Err(e.into());
                }
            };

            // ── 3. Collect tool definitions ──────────────────────────────────
            // Get all tool definitions that are currently available to the agent.
            let tool_defs: Vec<_> = self.dispatcher.definitions().cloned().collect();

            // ── 3b. Emit PreTurnReady confirmation ───────────────────────────
            // Estimate the number of tokens to be sent in the prompt request.
            // We use a simple heuristic where 4 characters roughly equal 1 token.
            // This is useful for detecting and debugging context window overflow issues.
            let estimated_tokens = clean_messages
                .iter()
                .flat_map(|m| m.content.iter())
                .map(|block| match block {
                    ContentBlock::Text(t) => t.len() / 4,
                    ContentBlock::ToolCall(c) => c.arguments.to_string().len() / 4 + 10,
                    ContentBlock::ToolResult(r) => {
                        let content_len = match &r.content {
                            ToolContent::Text(t) => t.len(),
                        };
                        content_len / 4 + 10
                    }
                    _ => 5,
                })
                .sum::<usize>();

            // Let the frontend know that all pre-turn processing succeeded and we are
            // about to dispatch the API request to the model provider.
            let _ = self
                .event_tx
                .send(SessionEvent::PreTurnReady {
                    turn_index: self.turn_index,
                    message_count: clean_messages.len(),
                    tool_count: tool_defs.len(),
                    estimated_tokens,
                })
                .await;

            // ── 4. Build request body ────────────────────────────────────────
            // Construct the payload for the model provider request.
            let body = build_request(
                &self.config.provider_config.provider,
                self.config.provider_config.model_id(),
                self.config.provider_config.max_tokens(),
                &clean_messages,
                true, // streaming = true
            )?;

            // ── 5. Send + consume SSE stream ─────────────────────────────────
            // Clone to String so there's no borrow of self across the await.
            let base_url = self.config.provider_config.effective_base_url();
            let provider = &self.config.provider_config.provider;
            let endpoint = match provider {
                Provider::Anthropic => format!("{}/messages", base_url.trim_end_matches('/')),
                Provider::Gemini => format!("{}/models", base_url.trim_end_matches('/')),
                Provider::Cohere => format!("{}/chat", base_url.trim_end_matches('/')),
                _ => format!("{}/chat/completions", base_url.trim_end_matches('/')),
            };
            let api_key = self
                .config
                .provider_config
                .credentials
                .api_key
                .expose()
                .to_string();

            // Hey friend! We pass the current `turn_index` to `send_streaming` so that
            // the `StreamingTagDetector` can prefix the generated call IDs correctly.
            let mut stream_result = send_streaming(
                &self.http_client,
                &self.config.provider_config.provider,
                &endpoint,
                &api_key,
                body,
                &self.event_tx,
                &mut self.cmd_rx,
                &mut self.pending_commands,
                self.turn_index,
            )
            .await
            .map_err(|e| {
                self.lifecycle = LifecycleState::Failed;
                e
            })?;

            // ── Parse Plain-Text Tag Protocol ───────────────────────────────
            // We do a hard cut away from provider-native JSON tool calling.
            // All tool calls are parsed exactly once from the final assistant text.
            let parse_res = operon_tools_parser::parse(&stream_result.text);

            let mut parsed_calls = Vec::new();
            for (idx, raw_call) in parse_res.calls.into_iter().enumerate() {
                // Generate a unique ToolCallId for this session + turn + call index.
                let call_id = ToolCallId(format!("{}-{}-{}", self.session_id, self.turn_index, idx));
                parsed_calls.push(raw_call.into_tool_call(call_id));
            }

            stream_result.text = parse_res.text;
            stream_result.tool_calls = parsed_calls;

            // Emit ToolCallStart and ToolCallArgsReady events so the UI updates properly.
            for call in &stream_result.tool_calls {
                // Build display args: serialize everything except __body__ as JSON,
                // then append __body__ raw with real newlines so all UIs can render it
                // as a preformatted block without JSON-unescaping.
                let mut display_map = call.arguments.as_object()
                    .cloned()
                    .unwrap_or_default();
                let raw_body = display_map.remove("__body__")
                    .and_then(|v| v.as_str().map(|s| s.to_string()));

                let mut args_display = if display_map.is_empty() {
                    String::new()
                } else {
                    serde_json::to_string(&serde_json::Value::Object(display_map))
                        .unwrap_or_default()
                };

                if let Some(body) = raw_body {
                    if !args_display.is_empty() {
                        args_display.push('\n');
                    }
                    args_display.push_str("__body__:\n");
                    args_display.push_str(&body);
                }

                let _ = self.event_tx.send(SessionEvent::ToolCallStart {
                    call_id: call.id.0.clone(),
                    name: call.name.clone(),
                }).await;
                let _ = self.event_tx.send(SessionEvent::ToolCallArgsReady {
                    call_id: call.id.0.clone(),
                    name: call.name.clone(),
                    args_json: args_display,
                }).await;
            }

            // ── 6. Record token usage + emit TokenUsageUpdated ───────────────
            // Update the session token state from the usage metadata in the stream.
            // Emit a TokenUsageUpdated event so the TUI status bar stays current.
            if let Some(usage_raw) = &stream_result.usage_raw {
                if let Some(record) = session::events::extract_usage_record(
                    usage_raw,
                    self.config.provider_config.model_id(),
                    &format!("{:?}", self.config.provider_config.provider),
                ) {
                    self.token_state.record_turn(&record);

                    let _ = self
                        .event_tx
                        .send(SessionEvent::TokenUsageUpdated {
                            input_tokens: record.input_tokens,
                            output_tokens: record.output_tokens,
                            context_total: self.token_state.current_context_tokens,
                            cache_read_tokens: record.cache_read_tokens,
                            cache_write_tokens: record.cache_write_tokens,
                        })
                        .await;

                    self.emit_context_usage_update().await;
                }
            }

            // ── 7. Push assistant message into history ───────────────────────
            let assistant_message = session::build_assistant_message(&stream_result);
            self.messages.push(assistant_message);

            // ── 8. Persist turn ──────────────────────────────────────────────
            if let Some(store) = &self.store {
                store
                    .save_turn(
                        &self.session_id,
                        self.turn_index,
                        &self.messages,
                        Some(self.token_state.current_context_tokens),
                    )
                    .await?;
            }

            // Emit TurnComplete so the UI can update turn counters.
            let _ = self
                .event_tx
                .send(SessionEvent::TurnComplete {
                    turn_index: self.turn_index,
                })
                .await;
            self.turn_index += 1;

            // ── 9. No tool calls → loop is done ─────────────────────────────
            if stream_result.tool_calls.is_empty() {
                let _ = self.event_tx.send(SessionEvent::Done).await;
                self.lifecycle = LifecycleState::Done;
                break;
            }

            // ── 10. Check for user cancellation ─────────────────────────────
            // Drain any queued commands first so we do not accidentally drop a
            // pending Approve/Deny while checking for Cancel.
            session::commands::drain(&mut self.cmd_rx, &mut self.pending_commands);
            let mut should_stop = session::commands::take_matching(&mut self.pending_commands, None).is_some();
            if should_stop {
                tracing::info!("Session cancelled by user command");
            }

            // ── 11. Policy-check and dispatch tool calls sequentially ────────
            // Tool calls are still processed in the order the model emitted them.
            // Do NOT parallelize — order matters for read-ledger enforcement.
            let mut tool_results: Vec<ContentBlock> = Vec::new();

            for call in stream_result.tool_calls {
                // If the user cancelled after the previous call, stop scheduling
                // any new tool work and exit the session cleanly after preserving
                // any results that were already produced.
                if should_stop {
                    break;
                }

                // Give any already-buffered commands a chance to stop the loop
                // before we spend time dispatching the next call.
                session::commands::drain(&mut self.cmd_rx, &mut self.pending_commands);
                if session::commands::take_matching(&mut self.pending_commands, None).is_some() {
                    tracing::info!("Session cancelled by user command");
                    should_stop = true;
                    break;
                }

                // ── ask tool: intercept before policy check ──────────────────────────
                if call.name == "ask" {
                    match session::ask::handle_ask_intercept(
                        &call,
                        &self.event_tx,
                        &mut self.cmd_rx,
                        &mut self.pending_commands,
                    )
                    .await {
                        session::ask::AskInterceptOutcome::Responded(result) => {
                            tool_results.push(ContentBlock::ToolResult(result));
                            continue;
                        }
                        session::ask::AskInterceptOutcome::Cancelled => {
                            should_stop = true;
                            break;
                        }
                    }
                }

                // Policy check + dispatch
                match session::dispatch::handle_tool_call(
                    call,
                    &self.policy_resolver,
                    self.caller_role(),
                    &mut self.dispatcher,
                    &self.event_tx,
                    &mut self.cmd_rx,
                    &mut self.pending_commands,
                )
                .await {
                    session::dispatch::DispatchOutcome::Result(result) => {
                        tool_results.push(ContentBlock::ToolResult(result));
                    }
                    session::dispatch::DispatchOutcome::Denied(result) => {
                        tool_results.push(ContentBlock::ToolResult(result));
                    }
                    session::dispatch::DispatchOutcome::Cancelled => {
                        should_stop = true;
                        break;
                    }
                }
            }

            // Push all tool results as a single Tool-role message.
            if !tool_results.is_empty() {
                self.messages.push(ConversationMessage {
                    role: MessageRole::Tool,
                    content: tool_results,
                    stop_reason: None,
                });
            }

            if should_stop {
                let _ = self.event_tx.send(SessionEvent::Done).await;
                self.lifecycle = LifecycleState::Done;
                break;
            }

            // Loop back to step 1.
        }

        Ok(())
    }

    /// Pause the session (only valid while `Running`).
    pub fn pause(&mut self) -> Result<(), SessionError> {
        if !self.lifecycle.can_pause() {
            return Err(SessionError::InvalidState {
                state: format!("{:?}", self.lifecycle),
            });
        }
        self.lifecycle = LifecycleState::Paused;
        Ok(())
    }

    /// Resume a paused session with a new user message.
    pub async fn resume(&mut self, user_message: String) -> Result<(), SessionError> {
        self.run(user_message).await
    }

    /// Returns the current session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Returns the current lifecycle state.
    pub fn lifecycle(&self) -> &LifecycleState {
        &self.lifecycle
    }

    /// Returns a reference to the resolved PolicyConfig for this session.
    ///
    /// Useful for the TUI to display which directories are currently accessible.
    pub fn policy(&self) -> &PolicyConfig {
        &self.config.policy
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod runner_tests;
