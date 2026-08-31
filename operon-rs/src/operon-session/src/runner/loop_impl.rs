// runner/loop_impl.rs — The per-turn agent loop.
//
// Contains the `run()` method on `SessionRunner`. This is the core agentic
// cycle: compaction check → snapshot → sanitize → collect tools → build
// request → stream → record usage → push assistant message → persist →
// check for tool calls → dispatch (delegated to tool_dispatch.rs) → loop.

use operon_context::{
    sanitize, ContentBlock, ConversationMessage, MessageRole, ToolContent, UsageRecord,
};
use operon_events::SessionEvent;
use operon_providers::Provider;

use crate::error::SessionError;
use crate::http::send_streaming;
use crate::lifecycle::LifecycleState;
use crate::request::build_request;

use super::message_build::{build_assistant_message, build_user_message, extract_usage_record};
use super::tool_dispatch::ToolCallFlow;
use super::SessionRunner;

impl SessionRunner {
    /// Run one user turn through the agent loop.
    ///
    /// Pushes `user_message` and optional attached images/files into the conversation, then loops:
    ///   1. Check compaction threshold → compact if needed
    ///   2. Build snapshot + sanitize
    ///   3. Collect tool definitions
    ///   4. Build request + stream
    ///   5. Record token usage + emit TokenUsageUpdated + ContextUsageUpdated
    ///   6. Push assistant message into history
    ///   7. Persist turn to SQLite
    ///   8. If no tool calls → emit Done + break
    ///   9. Check for Cancel command
    ///  10. Policy-check each tool call; Ask waits, Deny blocks
    ///  11. Dispatch allowed tool calls sequentially + stream progress events
    ///  12. Loop back for the next model turn
    pub async fn run(
        &mut self,
        user_message: String,
        image_blocks: Vec<ContentBlock>,
        file_paths: Vec<std::path::PathBuf>,
    ) -> Result<(), SessionError> {
        // Guard: only Idle and Paused sessions may enter the loop.
        if !self.lifecycle.can_run() {
            return Err(SessionError::InvalidState {
                state: format!("{:?}", self.lifecycle),
            });
        }
        self.lifecycle = LifecycleState::Running;

        let mut turn_start_len = self.messages.len();

        // Push the user's message into the conversation history.
        let user_blocks = build_user_message(&user_message, image_blocks, &file_paths);
        self.messages.push(ConversationMessage::user(user_blocks));

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
                    Ok(()) => {
                        // Compaction rebuilt self.messages into [system, summary, preserved..., in_flight_user_msg].
                        // Reset turn_start_len to the index of the in-flight user message so slice indexing is always valid.
                        turn_start_len = self.messages.len().saturating_sub(1);
                    }
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
                            ToolContent::Json(val) => val.to_string().len(),
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
                &tool_defs,
                true, // streaming = true
            )?;

            // ── 5. Send + consume SSE stream ─────────────────────────────────
            // Clone to String so there's no borrow of self across the await.
            let base_url = self.config.provider_config.effective_base_url();
            let provider = &self.config.provider_config.provider;
            let endpoint = match provider {
                Provider::Anthropic => format!("{}/messages", base_url.trim_end_matches('/')),
                Provider::Gemini => {
                    let model_id = self.config.provider_config.model_id();
                    let clean_id = model_id.strip_prefix("models/").unwrap_or(model_id);
                    format!(
                        "{}/models/{}:streamGenerateContent?alt=sse",
                        base_url.trim_end_matches('/'),
                        clean_id
                    )
                }
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

            let stream_result = send_streaming(
                &self.http_client,
                &self.config.provider_config.provider,
                &endpoint,
                &api_key,
                body,
                &self.event_tx,
                &mut self.cmd_rx,
                &mut self.pending_commands,
            )
            .await
            .inspect_err(|_e| {
                self.lifecycle = LifecycleState::Failed;
            })?;

            // ── 6. Record token usage + emit TokenUsageUpdated ───────────────
            // Update the session token state from the usage metadata in the stream.
            // If the provider omitted usage metadata, estimate token usage from message lengths as a reliable fallback.
            let usage_record = stream_result
                .usage_raw
                .as_ref()
                .and_then(|raw| {
                    extract_usage_record(
                        raw,
                        self.config.provider_config.model_id(),
                        &format!("{:?}", self.config.provider_config.provider),
                    )
                })
                .unwrap_or_else(|| {
                    let prompt_chars: usize = self
                        .messages
                        .iter()
                        .map(|m| {
                            m.content
                                .iter()
                                .map(|b| match b {
                                    ContentBlock::Text(t) => t.len(),
                                    ContentBlock::Reasoning(r) => r.thinking.len(),
                                    _ => 100,
                                })
                                .sum::<usize>()
                        })
                        .sum();
                    let output_chars = stream_result.text.len()
                        + stream_result
                            .reasoning
                            .as_ref()
                            .map(|r| r.thinking.len())
                            .unwrap_or(0);
                    let input_tokens = (prompt_chars / 4).max(1);
                    let output_tokens = (output_chars / 4).max(1);
                    UsageRecord {
                        input_tokens,
                        output_tokens,
                        cache_read_tokens: None,
                        cache_write_tokens: None,
                        model: self.config.provider_config.model_id().to_string(),
                        provider: format!("{:?}", self.config.provider_config.provider),
                    }
                });

            self.token_state.record_turn(&usage_record);

            let _ = self
                .event_tx
                .send(SessionEvent::TokenUsageUpdated {
                    input_tokens: usage_record.input_tokens,
                    output_tokens: usage_record.output_tokens,
                    context_total: self.token_state.current_context_tokens,
                    cache_read_tokens: usage_record.cache_read_tokens,
                    cache_write_tokens: usage_record.cache_write_tokens,
                })
                .await;

            self.emit_context_usage_update().await;

            // ── 7. Push assistant message into history ───────────────────────
            let assistant_message = build_assistant_message(&stream_result);
            self.messages.push(assistant_message);

            // ── 8. Persist turn and todos ───────────────────────────────────
            if let Some(store) = &self.store {
                let turn_messages = if turn_start_len < self.messages.len() {
                    &self.messages[turn_start_len..]
                } else {
                    &self.messages[..]
                };
                store
                    .save_turn(
                        &self.session_id,
                        self.turn_index,
                        turn_messages,
                        Some(self.token_state.current_context_tokens),
                    )
                    .await?;

                // Hey friend! We also persist the current session's todo list to disk.
                // This ensures any todo items created or updated during this turn are
                // permanently saved in the session JSON file so they survive across turns.
                store
                    .save_todos(&self.session_id, &self.dispatcher.todo_store().list())
                    .await?;
            }

            // ── 9. No tool calls → loop is done ─────────────────────────────
            if stream_result.tool_calls.is_empty() {
                let _ = self
                    .event_tx
                    .send(SessionEvent::TurnComplete {
                        turn_index: self.turn_index,
                    })
                    .await;
                self.turn_index += 1;
                let _ = self.event_tx.send(SessionEvent::Done).await;
                self.lifecycle = LifecycleState::Done;
                break;
            }

            // ── 10. Check for user cancellation ─────────────────────────────
            // Drain any queued commands first so we do not accidentally drop a
            // pending Approve/Deny while checking for Cancel.
            self.drain_ready_commands();
            let mut should_stop = self.take_matching_command(None).is_some();
            if should_stop {
                tracing::info!("Session cancelled by user command");
                self.turn_index += 1;
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
                self.drain_ready_commands();
                if self.take_matching_command(None).is_some() {
                    tracing::info!("Session cancelled by user command");
                    should_stop = true;
                    break;
                }

                // Delegate to handle_tool_call for ask-tool interception,
                // policy gating, and dispatcher invocation.
                match self.handle_tool_call(call, &mut tool_results).await {
                    ToolCallFlow::Continue => {}
                    ToolCallFlow::Stop => {
                        should_stop = true;
                        break;
                    }
                }
            }

            // Hey friend! If any tool calls ran, immediately sync the latest todo state to disk.
            if let Some(store) = &self.store {
                let _ = store
                    .save_todos(&self.session_id, &self.dispatcher.todo_store().list())
                    .await;
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
}
