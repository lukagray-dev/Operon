// runner.rs — The Operon agent loop.
//
// `SessionRunner` is the central component of the session crate. It owns:
//   - The conversation message history (Vec<ConversationMessage>)
//   - The HTTP client for provider requests
//   - The tool dispatcher for tool call routing
//   - The snapshot builder for per-turn system prompt generation
//   - The token tracker for compaction threshold monitoring
//   - The compaction client for context summarization
//   - The SQLite store for turn persistence (optional)
//   - The lifecycle state machine
//   - The inbound command channel (SessionCommand from the UI)
//
// The runner does NOT own:
//   - Wire format logic (operon-context-normalize-*)
//   - Tool implementations (operon-tools)
//   - Compaction algorithm (operon-context-compaction)
//
// The agent loop (run()) implements the following cycle:
//   1. Compaction check (compact if token budget exceeded)
//   2. Build snapshot + sanitize messages
//   3. Collect tool definitions
//   4. Build request body
//   5. Send + consume SSE stream → events
//   6. Record token usage + emit TokenUsageUpdated + ContextUsageUpdated
//   7. Push assistant message into history
//   8. Persist turn to SQLite
//   9. If no tool calls → Done; break
//  10. Check for Cancel command
//  11. Policy-check each tool call; Ask pauses for approval, Deny blocks it
//  12. Dispatch allowed tool calls sequentially → emit ToolDegraded if needed
//  13. Loop back for the next model turn
//
// ── Project directory model ───────────────────────────────────────────────────
//
// When a project directory is open (config.project_dir is Some), the runner uses
// it as the snapshot root (workspace_root) so AGENTS.md, directory tree, and git
// status come from the project instead of ~/.operon/workspace/. No policy changes
// are made at runtime — the project directory must already be in config.toml as a
// normal allowed directory, configured by the user via the Permissions settings.

use std::collections::VecDeque;
use std::sync::Arc;

use reqwest::Client;
use tokio::sync::mpsc;

use operon_config::PolicyConfig;
use operon_context::{
    compact, sanitize, AnthropicCompactionClient, ContentBlock, ConversationMessage, MessageRole,
    Role, SessionTokenState, SnapshotBuilder, TokenBudget, ToolCall, ToolContent, ToolResult,
    UsageRecord,
};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::{CallerRole, PolicyDecision, PolicyResolver};
use operon_providers::Provider;
use operon_tools::{dispatcher::Dispatcher, ToolProgressEmitter};
// Hey friend! We import AskArgs and AskOutput so we can parse input arguments and format tool outputs.
use operon_tools_ask::{AskArgs, AskOutput};

use crate::config::SessionConfig;
use crate::error::SessionError;
use crate::http::{send_streaming, StreamResult};
use crate::lifecycle::LifecycleState;
use crate::request::build_request;
use crate::store::SessionStore;

// ─────────────────────────────────────────────────────────────────────────────
// SessionRunner
// ─────────────────────────────────────────────────────────────────────────────

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
    session_id: String,
    /// Runtime configuration (provider, model, tool groups, policy, etc.).
    config: SessionConfig,
    /// The full conversation history including all turns in this session.
    messages: Vec<ConversationMessage>,
    /// Tool dispatcher — routes tool calls to implementations.
    dispatcher: Dispatcher,
    /// Snapshot builder — generates the system prompt block per turn.
    snapshot_builder: SnapshotBuilder,
    /// Per-session exact token state (updated from API usage blocks).
    token_state: SessionTokenState,
    /// Immutable token budget for the model's context window.
    token_budget: TokenBudget,
    /// Current lifecycle state machine.
    lifecycle: LifecycleState,
    /// Shared HTTP client (clone-able for the compaction client).
    http_client: Client,
    /// Outbound event channel — UI/tests receive from the other end.
    event_tx: mpsc::Sender<SessionEvent>,
    /// Inbound command channel — UI sends Cancel/Approve/Deny into the loop.
    cmd_rx: mpsc::Receiver<SessionCommand>,
    /// Policy engine for permission checks before tool dispatch.
    policy_resolver: PolicyResolver,
    /// Buffered inbound commands that arrived before the runner was waiting.
    pending_commands: VecDeque<SessionCommand>,
    /// Optional SQLite store for turn persistence.
    store: Option<SessionStore>,
    /// 0-based index of the next turn to execute.
    turn_index: usize,
}

impl SessionRunner {
    /// Create a new session runner. Does not start the loop.
    ///
    /// # Parameters
    ///
    /// - `config` — runtime configuration (consumed; `mut` so project_dir can be injected).
    /// - `event_tx` — outbound channel. The caller keeps the `Receiver` end.
    /// - `cmd_rx` — inbound command channel. The caller keeps the `Sender` end.
    ///
    /// # Initialization order
    ///
    /// 1. Generate unique session ID.
    /// 2. Build `SnapshotBuilder` (starts filesystem watcher).
    /// 3. Register tool groups on `Dispatcher`.
    /// 4. Open SQLite store if configured.
    /// 5. Emit `SessionStarted` — UI receives this immediately after construction.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the snapshot builder fails, or the SQLite store cannot be opened.
    pub async fn new(
        config: SessionConfig,
        event_tx: mpsc::Sender<SessionEvent>,
        cmd_rx: mpsc::Receiver<SessionCommand>,
    ) -> Result<Self, SessionError> {
        // Determine the session ID:
        // 1. If a database path is provided, check if it contains an existing session ID in its record.
        // 2. If it is a new database, use the file stem name as the session ID.
        // 3. If no database path is provided (e.g. testing), generate a unique timestamp-based ID.
        let mut session_id = generate_session_id();
        let mut store = None;

        if let Some(path) = &config.store_path {
            let s = SessionStore::open(path).await?;
            let existing_id = if let Ok(rows) = s.list_sessions().await {
                rows.first().map(|r| r.id.clone())
            } else {
                None
            };

            if let Some(id) = existing_id {
                session_id = id;
            } else {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    session_id = stem.to_string();
                }
                s.create_session(
                    &session_id,
                    &config.workspace_root.display().to_string(),
                    config.provider_config.model_id(),
                    &format!("{:?}", config.provider_config.provider),
                )
                .await?;
            }
            store = Some(s);
        }

        // Build the snapshot builder — this also starts the filesystem watcher.
        let snapshot_config = config.snapshot_config(&session_id);
        let snapshot_builder = SnapshotBuilder::new(snapshot_config)?;

        // Initialize the dispatcher and register the "load_tools" meta-tool.
        let mut dispatcher = Dispatcher::new();
        dispatcher.register_load_tool();

        // Register tool groups based on the session configuration.
        for group in &config.tool_groups {
            match group.as_str() {
                "fs" => dispatcher.register_fs_tools(),
                "shell" => dispatcher.register_shell_tools(),
                "web" => dispatcher.register_web_tools(),
                "todo" => dispatcher.register_todo_tools(),
                "ask" => dispatcher.register_ask_tool(),
                other => tracing::warn!("Unknown tool group: {other}"),
            }
        }

        // Build the token budget from the provider config's context window size.
        let token_budget = TokenBudget::with_window(config.provider_config.context_window())
            .map_err(|e| SessionError::Stream(e.to_string()))?;

        // Build the policy resolver from the fully validated policy config.
        let policy_resolver = PolicyResolver::new(config.policy.clone());

        // Emit SessionStarted — the UI now knows the session ID and can label panels.
        // This is the first event on the channel; it fires before any turn runs.
        let _ = event_tx
            .send(SessionEvent::SessionStarted {
                session_id: session_id.clone(),
            })
            .await;

        let _ = event_tx.send(context_usage_event(&token_budget, 0)).await;

        Ok(Self {
            session_id,
            config,
            messages: Vec::new(),
            dispatcher,
            snapshot_builder,
            token_state: SessionTokenState::new(),
            token_budget,
            lifecycle: LifecycleState::Idle,
            http_client: Client::new(),
            event_tx,
            cmd_rx,
            policy_resolver,
            pending_commands: VecDeque::new(),
            store,
            turn_index: 0,
        })
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
                        // The output content of load_tools is returned as a JSON structure.
                        if let ToolContent::Json(ref json) = result.content {
                            // Inside that JSON, the "group" key specifies which group was loaded
                            // (for example: { "group": "fs", "tool_count": 7, "tools": [...] }).
                            if let Some(group) = json.get("group").and_then(|v| v.as_str()) {
                                // Mark this group as loaded in the dispatcher!
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
    ///
    /// Pushes `user_message` into the conversation, then loops:
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
    ///  11. Dispatch allowed tool calls sequentially + emit ToolDegraded if needed
    ///  12. Loop back for the next model turn
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
            .map_err(|e| {
                self.lifecycle = LifecycleState::Failed;
                e
            })?;

            // ── 6. Record token usage + emit TokenUsageUpdated ───────────────
            // Update the session token state from the usage metadata in the stream.
            // Emit a TokenUsageUpdated event so the TUI status bar stays current.
            if let Some(usage_raw) = &stream_result.usage_raw {
                if let Some(record) = extract_usage_record(
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
            let assistant_message = build_assistant_message(&stream_result);
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
            self.drain_ready_commands();
            let mut should_stop = self.take_matching_command(None).is_some();
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
                self.drain_ready_commands();
                if self.take_matching_command(None).is_some() {
                    tracing::info!("Session cancelled by user command");
                    should_stop = true;
                    break;
                }

                // ── ask tool: intercept before policy check ──────────────────────────
                // Hey friend! The ask tool is unique. It suspends the loop and waits for the
                // user's answer on the command channel, bypassing the dispatcher entirely.
                if call.name == "ask" {
                    let ask_id = call.id.0.clone();

                    // Hey friend! We parse and validate the arguments before suspending the loop.
                    // If parsing fails (for example, if the options count is incorrect), we return
                    // an error ToolResult immediately without suspending.
                    let ask_result = match AskArgs::from_json(&call.arguments) {
                        Err(reason) => {
                            let result = ToolResult {
                                call_id: call.id.clone(),
                                name: "ask".to_string(),
                                content: ToolContent::Text(reason.to_string()),
                                is_error: true,
                            };
                            let _ = self
                                .event_tx
                                .send(SessionEvent::ToolCallResult {
                                    call_id: ask_id.clone(),
                                    name: "ask".to_string(),
                                    is_error: true,
                                    content_json: tool_result_content_json(&result),
                                })
                                .await;
                            tool_results.push(ContentBlock::ToolResult(result));
                            continue;
                        }
                        Ok(args) => args,
                    };

                    // Emit AskQuestion event. The frontend UI will receive this and render
                    // the multiple-choice question widget to the user.
                    let _ = self
                        .event_tx
                        .send(SessionEvent::AskQuestion {
                            id: ask_id.clone(),
                            question: ask_result.question.clone(),
                            options: ask_result.options.to_vec(),
                        })
                        .await;

                    // Suspend the loop and block here until we receive the answer command or a cancel command.
                    let answer = loop {
                        match self.wait_for_relevant_command(Some(&ask_id)).await {
                            SessionCommand::AskResponse { id, answer } if id == ask_id => {
                                break answer;
                            }
                            SessionCommand::Cancel => {
                                should_stop = true;
                                break String::new();
                            }
                            _ => continue,
                        }
                    };

                    if should_stop {
                        break;
                    }

                    // Hey friend! We pack the user's answer into a structured AskOutput,
                    // serialize it to a JSON value, and build the ToolResult which will
                    // be passed back to the AI model.
                    let content = ToolContent::Json(
                        serde_json::to_value(AskOutput { answer })
                            .expect("AskOutput serialization should never fail"),
                    );
                    let result = ToolResult {
                        call_id: call.id.clone(),
                        name: "ask".to_string(),
                        content: content.clone(),
                        is_error: false,
                    };
                    let _ = self
                        .event_tx
                        .send(SessionEvent::ToolCallResult {
                            call_id: ask_id.clone(),
                            name: "ask".to_string(),
                            is_error: false,
                            content_json: tool_result_content_json(&result),
                        })
                        .await;
                    tool_results.push(ContentBlock::ToolResult(result));
                    continue; // Skip the rest of the loop body (no dispatcher call needed)
                }

                // Policy gate: Ask / Deny / Allow are handled before dispatch.
                match self.policy_resolver.check(&call, self.caller_role()) {
                    PolicyDecision::Allow => {
                        // Nothing special here — fall through to the dispatcher below.
                    }
                    PolicyDecision::Ask { reason } => {
                        let approval_id = call.id.0.clone();
                        let path = policy_path_for_call(&call);
                        let approval_path = path.clone();
                        let args_json = serde_json::to_string(&call.arguments).unwrap_or_default();

                        tracing::info!(
                            tool = %call.name,
                            approval_id = %approval_id,
                            reason = %reason,
                            "Tool call requires approval"
                        );

                        let _ = self
                            .event_tx
                            .send(SessionEvent::ApprovalRequired {
                                id: approval_id.clone(),
                                tool: call.name.clone(),
                                path,
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
                                        path: approval_path,
                                    })
                                    .await;

                                tracing::info!(
                                    tool = %call.name,
                                    approval_id = %approval_id,
                                    "Approval granted; dispatching tool call"
                                );
                            }
                            SessionCommand::Deny { id } if id == approval_id => {
                                tracing::info!(
                                    tool = %call.name,
                                    approval_id = %approval_id,
                                    "Approval denied by the user"
                                );

                                let path = policy_path_for_call(&call);
                                let _ = self
                                    .event_tx
                                    .send(SessionEvent::PermissionDenied {
                                        tool: call.name.clone(),
                                        path,
                                        reason: "approval denied by the user".to_string(),
                                    })
                                    .await;

                                let result = opaque_permission_denied_result(&call);
                                let _ = self
                                    .event_tx
                                    .send(SessionEvent::ToolCallResult {
                                        call_id: result.call_id.0.clone(),
                                        name: result.name.clone(),
                                        is_error: result.is_error,
                                        content_json: tool_result_content_json(&result),
                                    })
                                    .await;

                                tool_results.push(ContentBlock::ToolResult(result));
                                continue;
                            }
                            SessionCommand::Cancel => {
                                tracing::info!(
                                    tool = %call.name,
                                    approval_id = %approval_id,
                                    "Session cancelled while waiting for approval"
                                );
                                should_stop = true;
                                break;
                            }
                            _ => {
                                tracing::warn!(
                                    tool = %call.name,
                                    approval_id = %approval_id,
                                    "Unexpected command returned while waiting for approval"
                                );
                                continue;
                            }
                        }
                    }
                    PolicyDecision::Deny { reason } => {
                        let path = policy_path_for_call(&call);
                        tracing::warn!(
                            tool = %call.name,
                            ?path,
                            reason = %reason,
                            "Tool call denied by policy"
                        );

                        let _ = self
                            .event_tx
                            .send(SessionEvent::PermissionDenied {
                                tool: call.name.clone(),
                                path,
                                reason,
                            })
                            .await;

                        let result = opaque_permission_denied_result(&call);
                        let _ = self
                            .event_tx
                            .send(SessionEvent::ToolCallResult {
                                call_id: result.call_id.0.clone(),
                                name: result.name.clone(),
                                is_error: result.is_error,
                                content_json: tool_result_content_json(&result),
                            })
                            .await;

                        tool_results.push(ContentBlock::ToolResult(result));
                        continue;
                    }
                }

                // dispatch_with_progress() returns DispatchOutcome so we can observe degradation
                // while forwarding runtime progress events to the UI.
                let progress_emitter = self.tool_progress_emitter();
                let outcome = self
                    .dispatcher
                    .dispatch_with_progress(call, Some(progress_emitter))
                    .await;

                // If this is the FIRST malformed call for this tool, emit ToolDegraded
                // so the TUI can show a warning badge on the tool.
                if let Some(ref name) = outcome.newly_degraded {
                    let _ = self
                        .event_tx
                        .send(SessionEvent::ToolDegraded { name: name.clone() })
                        .await;
                }

                let result = outcome.result;

                let content_json = tool_result_content_json(&result);

                let _ = self
                    .event_tx
                    .send(SessionEvent::ToolCallResult {
                        call_id: result.call_id.0.clone(),
                        name: result.name.clone(),
                        is_error: result.is_error,
                        content_json: content_json.clone(),
                    })
                    .await;

                tool_results.push(ContentBlock::ToolResult(result));
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

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Run context compaction: summarize old history and rebuild the message array.
    async fn run_compaction(&mut self) -> Result<(), SessionError> {
        let snapshot = self.snapshot_builder.build()?;
        let tokens_before = self.token_state.current_context_tokens;

        match &self.config.provider_config.provider {
            Provider::Anthropic => {
                let compaction_client = AnthropicCompactionClient {
                    api_key: self
                        .config
                        .provider_config
                        .credentials
                        .api_key
                        .expose()
                        .to_string(),
                    model_id: self.config.provider_config.model_id().to_string(),
                    http: self.http_client.clone(),
                };

                let result = compact(
                    self.messages.clone(),
                    &snapshot,
                    &compaction_client,
                    &self.config.compaction,
                    tokens_before,
                )
                .await?;

                self.messages = result.messages;
                self.token_state.reset();
                self.dispatcher.notify_compaction();

                let _ = self
                    .event_tx
                    .send(SessionEvent::CompactionOccurred {
                        tokens_before,
                        tokens_after: result.tokens_after,
                    })
                    .await;

                self.emit_context_usage_update().await;
            }
            other => {
                tracing::warn!(
                    "Context compaction not supported for provider {:?} — skipping",
                    other
                );
                let _ = self
                    .event_tx
                    .send(SessionEvent::Warning {
                        message: format!(
                            "Compaction not supported for provider {:?}",
                            self.config.provider_config.provider
                        ),
                    })
                    .await;
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// Build a `ConversationMessage` from a fully assembled `StreamResult`.
fn build_assistant_message(result: &StreamResult) -> ConversationMessage {
    let mut blocks: Vec<ContentBlock> = Vec::new();

    // Hey friend! If the model did some reasoning/thinking during this turn, we prepend it as the
    // very first block in the message. This ensures the thinking block resides before the text or tool
    // blocks, which matches the model's actual execution flow and keeps providers like Anthropic happy!
    if let Some(reasoning) = &result.reasoning {
        blocks.push(ContentBlock::Reasoning(reasoning.clone()));
    }

    if !result.text.is_empty() {
        blocks.push(ContentBlock::Text(result.text.clone()));
    }

    for call in &result.tool_calls {
        blocks.push(ContentBlock::ToolCall(call.clone()));
    }

    let mut msg = ConversationMessage::assistant(blocks);

    if let Some(stop) = &result.stop_reason {
        msg = msg.with_stop(stop.clone());
    }

    msg
}

/// Extract a `UsageRecord` from a raw usage metadata JSON value.
///
/// Handles both Anthropic and OpenAI usage shapes:
///   - Anthropic: `{ "input_tokens": N, "output_tokens": N, "cache_read_input_tokens": N, ... }`
///   - OpenAI:    `{ "prompt_tokens": N, "completion_tokens": N }`
///
/// Returns `None` if the required fields are absent.
fn extract_usage_record(
    raw: &serde_json::Value,
    model_id: &str,
    provider_name: &str,
) -> Option<UsageRecord> {
    let input = raw
        .get("input_tokens")
        .or_else(|| raw.get("prompt_tokens"))
        .and_then(|v| v.as_u64())? as usize;

    let output = raw
        .get("output_tokens")
        .or_else(|| raw.get("completion_tokens"))
        .and_then(|v| v.as_u64())? as usize;

    let cache_read = raw
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let cache_write = raw
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    Some(UsageRecord {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_write_tokens: cache_write,
        model: model_id.to_string(),
        provider: provider_name.to_string(),
    })
}

/// Generate a unique session ID using the current nanosecond timestamp in hex.
fn generate_session_id() -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    format!("{nanos:x}")
}

/// Build the context gauge event from the current token state.
fn context_usage_event(token_budget: &TokenBudget, current_context_tokens: usize) -> SessionEvent {
    let context_window = token_budget.context_window();
    let remaining_context_tokens = context_window.saturating_sub(current_context_tokens);

    SessionEvent::ContextUsageUpdated {
        current_context_tokens,
        context_window,
        remaining_context_tokens,
        utilization: token_budget.utilization(current_context_tokens),
        compaction_limit: token_budget.compaction_limit(),
    }
}

#[cfg(test)]
#[path = "runner_tests.rs"]
mod runner_tests;

impl SessionRunner {
    /// Convert the session runtime role into the policy crate role.
    fn caller_role(&self) -> CallerRole {
        match self.config.role {
            Role::Owner => CallerRole::Owner,
            Role::External => CallerRole::External,
        }
    }

    /// Move any immediately available inbound commands into the local buffer.
    ///
    /// This prevents us from dropping commands when we only want to inspect
    /// whether a Cancel is pending before dispatching the next tool call.
    fn drain_ready_commands(&mut self) {
        while let Ok(command) = self.cmd_rx.try_recv() {
            self.pending_commands.push_back(command);
        }
    }

    /// Emit the current context-window gauge for the UI.
    async fn emit_context_usage_update(&self) {
        let _ = self
            .event_tx
            .send(context_usage_event(
                &self.token_budget,
                self.token_state.current_context_tokens,
            ))
            .await;
    }

    /// Build a synchronous progress callback that forwards tool progress into the event bus.
    ///
    /// The callback uses `try_send` so tool code can report progress without
    /// blocking on the async runtime.
    fn tool_progress_emitter(&self) -> ToolProgressEmitter {
        let event_tx = self.event_tx.clone();

        Arc::new(move |progress| {
            let _ = event_tx.try_send(SessionEvent::ToolProgress(progress));
        })
    }

    /// Remove the first buffered command that matches the requested approval.
    ///
    /// `approval_id = None` means only `Cancel` is relevant. When an approval ID
    /// is present, `Approve` and `Deny` must match that ID.
    fn take_matching_command(&mut self, approval_id: Option<&str>) -> Option<SessionCommand> {
        let index = self
            .pending_commands
            .iter()
            .position(|command| command_matches(command, approval_id))?;
        self.pending_commands.remove(index)
    }

    /// Wait until the command channel yields something relevant to the current
    /// approval request or a cancel signal.
    ///
    /// Irrelevant commands are buffered so we do not lose them.
    async fn wait_for_relevant_command(&mut self, approval_id: Option<&str>) -> SessionCommand {
        loop {
            if let Some(command) = self.take_matching_command(approval_id) {
                return command;
            }

            match self.cmd_rx.recv().await {
                Some(command) => self.pending_commands.push_back(command),
                None => return SessionCommand::Cancel,
            }
        }
    }
}

/// Return true if a buffered command should be consumed for the current state.
fn command_matches(command: &SessionCommand, approval_id: Option<&str>) -> bool {
    // Hey friend! Here we check if the command matches the expected command type.
    // A Cancel command is always matches. Approve, Deny, and AskResponse commands
    // match only if they carry the expected ID.
    match command {
        SessionCommand::Cancel => true,
        SessionCommand::Approve { id }
        | SessionCommand::Deny { id }
        | SessionCommand::AskResponse { id, .. } => {
            approval_id.is_some_and(|expected| expected == id)
        }
    }
}

/// Build the policy-facing path string for a tool call, if the tool uses one.
///
/// This helper extracts a representative filesystem path from the tool call's arguments.
/// The extracted path is used by the policy resolver to check whether the caller has
/// permission to access or operate on that specific path.
fn strip_range_suffix_str(s: &str) -> &str {
    if let Some(idx) = s.rfind(':') {
        let suffix = &s[idx + 1..];
        if suffix.eq_ignore_ascii_case("EOF")
            || suffix.parse::<usize>().is_ok()
            || (suffix.contains('-') && {
                let parts: Vec<&str> = suffix.split('-').collect();
                parts.len() == 2
                    && parts[0].parse::<usize>().is_ok()
                    && (parts[1].eq_ignore_ascii_case("EOF") || parts[1].parse::<usize>().is_ok())
            })
        {
            return &s[..idx];
        }
    }
    s
}

fn policy_path_for_call(call: &ToolCall) -> Option<String> {
    let raw_str = match call.name.as_str() {
        "read" => call
            .arguments
            .get("path")
            .and_then(|v| v.as_str())
            .or_else(|| {
                call.arguments.get("paths").and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.as_str()),
                    serde_json::Value::Array(arr) => arr.first().and_then(|item| item.as_str()),
                    _ => None,
                })
            }),

        "grep" => call
            .arguments
            .get("path")
            .and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.as_str()),
                serde_json::Value::Array(arr) => arr.first().and_then(|item| item.as_str()),
                _ => None,
            })
            .or_else(|| {
                call.arguments.get("paths").and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.as_str()),
                    serde_json::Value::Array(arr) => arr.first().and_then(|item| item.as_str()),
                    _ => None,
                })
            }),

        "ls" => call
            .arguments
            .get("path")
            .or_else(|| call.arguments.get("dir"))
            .and_then(|v| v.as_str()),

        "bash" => call.arguments.get("cwd").and_then(|v| v.as_str()),

        "write" | "edit" | "append" | "delete" => {
            call.arguments.get("path").and_then(|v| v.as_str())
        }

        _ => None,
    };

    raw_str.map(|s| strip_range_suffix_str(s).to_string())
}


/// Construct the opaque error result we return to the model when policy blocks a call.
fn opaque_permission_denied_result(call: &ToolCall) -> ToolResult {
    ToolResult {
        call_id: call.id.clone(),
        name: call.name.clone(),
        content: ToolContent::Text("Tool not available.".to_string()),
        is_error: true,
    }
}

/// Convert a tool result into the serialized content string emitted on the event bus.
fn tool_result_content_json(result: &ToolResult) -> String {
    match &result.content {
        ToolContent::Text(text) => text.clone(),
        ToolContent::Json(value) => value.to_string(),
    }
}
