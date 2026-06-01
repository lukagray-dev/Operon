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
//   6. Record token usage
//   7. Push assistant message into history
//   8. Persist turn to SQLite
//   9. If no tool calls → Done; break
//  10. Dispatch tool calls sequentially → push tool result messages → loop back

use reqwest::Client;
use tokio::sync::mpsc;

use operon_context_compaction::{
    compact, AnthropicCompactionClient,
};
use operon_context_normalize_messages::{
    ConversationMessage, ContentBlock, MessageRole,
};
use operon_context_normalize_tools::{
    Provider, ToolContent,
};
use operon_context_sanitizer::sanitize;
use operon_context_snapshot::SnapshotBuilder;
use operon_context_token_tracker::{SessionTokenState, TokenBudget, UsageRecord};
use operon_tools::dispatcher::Dispatcher;
use operon_events::SessionEvent;

use crate::config::SessionConfig;
use crate::error::SessionError;
use crate::http::{send_streaming, StreamResult};
use crate::lifecycle::LifecycleState;
use crate::request::{build_request, provider_endpoint};
use crate::store::SessionStore;

// ─────────────────────────────────────────────────────────────────────────────
// SessionRunner
// ─────────────────────────────────────────────────────────────────────────────

/// The Operon agent loop — owns all session state and drives the agentic cycle.
///
/// # Thread safety
///
/// `SessionRunner` is `Send` but not `Sync` (held in a single async task).
/// If the TUI or other components need concurrent access, wrap in `Arc<Mutex<SessionRunner>>`.
/// `SnapshotBuilder` itself is not `Sync`, so do not share the runner across threads directly.
///
/// # Lifecycle
///
/// 1. `SessionRunner::new(config, event_tx)` — create and initialize.
/// 2. `runner.run(user_message)` — enter the agent loop.
/// 3. Events flow over `event_tx` until `SessionEvent::Done` or `SessionEvent::Error`.
/// 4. `runner.pause()` / `runner.resume(msg)` — interrupt mid-session.
pub struct SessionRunner {
    /// Unique identifier for this session (hex nanoseconds).
    session_id: String,
    /// Runtime configuration (provider, model, tool groups, etc.).
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
    /// Optional SQLite store for turn persistence.
    store: Option<SessionStore>,
    /// 0-based index of the next turn to execute.
    turn_index: usize,
}

impl SessionRunner {
    /// Create a new session runner. Does not start the loop.
    ///
    /// Initializes all subsystems:
    /// - Generates a unique session ID
    /// - Builds the `SnapshotBuilder` for this workspace root
    /// - Registers tool groups on the `Dispatcher`
    /// - Opens the SQLite store if a path was provided
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] if the snapshot builder cannot be created
    /// (e.g. workspace root does not exist) or if the SQLite store cannot be opened.
    pub async fn new(
        config: SessionConfig,
        event_tx: mpsc::Sender<SessionEvent>,
    ) -> Result<Self, SessionError> {
        // Generate a unique session ID using nanosecond hex timestamp.
        // This is the same pattern used by SnapshotBuilder internally.
        let session_id = generate_session_id();

        // Build the snapshot builder — this also starts the filesystem watcher.
        let snapshot_config = config.snapshot_config(&session_id);
        let snapshot_builder = SnapshotBuilder::new(snapshot_config)?;

        // Initialize the dispatcher and register the "load_tools" meta-tool,
        // which is always available regardless of tool_groups configuration.
        let mut dispatcher = Dispatcher::new();
        dispatcher.register_load_tool();

        // Register tool groups based on the session configuration.
        // Unknown group names are logged as warnings and skipped safely.
        for group in &config.tool_groups {
            match group.as_str() {
                "fs"    => dispatcher.register_fs_tools(),
                "shell" => dispatcher.register_shell_tools(),
                "web"   => dispatcher.register_web_tools(),
                "todo"  => dispatcher.register_todo_tools(),
                other   => tracing::warn!("Unknown tool group: {other}"),
            }
        }

        // Build the token budget from the configured context window size.
        // Uses the default 90% compaction threshold.
        let token_budget = TokenBudget::with_window(config.context_window)
            .map_err(|e| SessionError::Stream(e.to_string()))?;

        // Open the SQLite store if a path was provided in the configuration.
        let store = if let Some(path) = &config.store_path {
            let s = SessionStore::open(path).await?;
            // Immediately create the session record so turns can reference it.
            s.create_session(
                &session_id,
                &config.workspace_root.display().to_string(),
                &config.model_id,
                &format!("{:?}", config.provider),
            )
            .await?;
            Some(s)
        } else {
            None
        };

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
            store,
            turn_index: 0,
        })
    }

    /// Run one user turn through the agent loop.
    ///
    /// Pushes `user_message` into the conversation, then loops:
    ///   1. Check compaction threshold → compact if needed
    ///   2. Build snapshot + sanitize
    ///   3. Collect tool definitions
    ///   4. Build request + stream
    ///   5. Record token usage
    ///   6. Push assistant message into history
    ///   7. Persist turn to SQLite
    ///   8. If no tool calls → emit Done + break
    ///   9. Dispatch tool calls sequentially → push tool result message → loop back
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] on any fatal failure. The lifecycle is set to
    /// `Failed` on error so the caller knows not to retry.
    pub async fn run(&mut self, user_message: String) -> Result<(), SessionError> {
        // Guard: only Idle and Paused sessions may enter the loop.
        if !self.lifecycle.can_run() {
            return Err(SessionError::InvalidState {
                state: format!("{:?}", self.lifecycle),
            });
        }
        self.lifecycle = LifecycleState::Running;

        // Push the user's message into the conversation history.
        self.messages.push(ConversationMessage::user(vec![
            ContentBlock::Text(user_message),
        ]));

        // The agent loop — continues until the model returns no tool calls.
        loop {
            // ── 1. Compaction check ──────────────────────────────────────────
            // Check if the token budget is exceeded before building the next request.
            // The guard in should_compact() ensures we only run compaction when needed.
            if self.token_budget.should_compact(self.token_state.current_context_tokens) {
                // Non-fatal: ThresholdNotReached is impossible here (we just checked),
                // but InsufficientHistory is possible on very short conversations.
                match self.run_compaction().await {
                    Ok(()) => {}
                    Err(SessionError::Compaction(
                        operon_context_compaction::CompactionError::ThresholdNotReached,
                    )) => {
                        // Shouldn't happen — we checked should_compact() above.
                        // Treat as a warning and continue.
                        tracing::warn!("Compaction triggered but threshold not reached — skipping");
                    }
                    Err(SessionError::Compaction(
                        operon_context_compaction::CompactionError::InsufficientHistory,
                    )) => {
                        // Not enough history to compact — this is not fatal.
                        let _ = self.event_tx
                            .send(SessionEvent::Warning {
                                message: "Context compaction skipped: insufficient history".to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        // Fatal compaction error — propagate upward.
                        self.lifecycle = LifecycleState::Failed;
                        return Err(e);
                    }
                }
            }

            // ── 2. Build snapshot + sanitize ─────────────────────────────────
            // The snapshot provides a fresh system prompt block for this turn.
            // Sanitize cleans the message array (orphans, role alternation, etc.)
            let snapshot = self.snapshot_builder.build()?;
            let clean_messages = sanitize(
                self.messages.clone(),
                &snapshot,
                self.config.role,
            )?;

            // ── 3. Collect tool definitions ──────────────────────────────────
            // The dispatcher returns short or detailed definitions per tool
            // depending on whether that tool has been degraded this session.
            let tool_defs: Vec<_> = self.dispatcher.definitions().cloned().collect();

            // ── 4. Build request body ────────────────────────────────────────
            let body = build_request(
                &self.config.provider,
                &self.config.model_id,
                self.config.max_tokens,
                &clean_messages,
                &tool_defs,
                true, // streaming = true
            )?;

            // ── 5. Send + consume SSE stream ─────────────────────────────────
            let endpoint = provider_endpoint(&self.config.provider);
            let stream_result = send_streaming(
                &self.http_client,
                &self.config.provider,
                endpoint,
                &self.config.api_key,
                body,
                &self.event_tx,
            )
            .await
            .map_err(|e| {
                self.lifecycle = LifecycleState::Failed;
                e
            })?;

            // ── 6. Record token usage ────────────────────────────────────────
            // Update the session token state from the usage metadata in the stream.
            // This gives us exact counts for the next compaction check.
            if let Some(usage_raw) = &stream_result.usage_raw {
                if let Some(record) = extract_usage_record(usage_raw, &self.config) {
                    self.token_state.record_turn(&record);
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

            // Emit a TurnComplete event so the UI can update turn counters.
            let _ = self
                .event_tx
                .send(SessionEvent::TurnComplete {
                    turn_index: self.turn_index,
                })
                .await;
            self.turn_index += 1;

            // ── 9. No tool calls → loop is done ─────────────────────────────
            // The model returned EndTurn (or equivalent) with no tool calls.
            // The agent loop exits naturally.
            if stream_result.tool_calls.is_empty() {
                let _ = self.event_tx.send(SessionEvent::Done).await;
                self.lifecycle = LifecycleState::Done;
                break;
            }

            // ── 10. Dispatch tool calls sequentially ─────────────────────────
            // Tool calls are dispatched in the order the model emitted them.
            // Do NOT parallelize — order matters for read-ledger enforcement.
            let mut tool_results: Vec<ContentBlock> = Vec::new();

            for call in stream_result.tool_calls {
                // Dispatch returns a ToolResult even on error — never panics.
                let result = self.dispatcher.dispatch(call).await;

                // Serialize the content for the event channel.
                let content_json = match &result.content {
                    ToolContent::Text(s) => s.clone(),
                    ToolContent::Json(v) => v.to_string(),
                };

                // Emit a ToolCallResult event so the UI can show the tool output.
                let _ = self
                    .event_tx
                    .send(SessionEvent::ToolCallResult {
                        call_id: result.call_id.0.clone(),
                        name: result.name.clone(),
                        is_error: result.is_error,
                        content_json: content_json.clone(),
                    })
                    .await;

                // Accumulate the result as a ContentBlock for the tool role message.
                tool_results.push(ContentBlock::ToolResult(result));
            }

            // Push all tool results as a single Tool-role message.
            // Providers that use a dedicated "tool" role (Anthropic) expect this grouping.
            self.messages.push(ConversationMessage {
                role: MessageRole::Tool,
                content: tool_results,
                stop_reason: None,
            });

            // Loop back to step 1 — the model will now see the tool results
            // and either issue more tool calls or return EndTurn.
        }

        Ok(())
    }

    /// Pause the session (only valid while `Running`).
    ///
    /// This is a logical pause — it does not cancel any in-flight HTTP request.
    /// Call from a separate task; the runner will check `can_pause()` at the
    /// next appropriate point. For a hard interrupt, drop the runner or kill the task.
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
    ///
    /// Equivalent to calling `run(user_message)` — the lifecycle guard in `run()`
    /// accepts `Paused` as a valid pre-run state.
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

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Run context compaction: summarize old history and rebuild the message array.
    ///
    /// Only constructs the `AnthropicCompactionClient` when the configured provider
    /// is Anthropic. For other providers, compaction is not yet supported and a
    /// warning is emitted. This is a temporary limitation until the compaction crate
    /// supports provider-agnostic clients.
    ///
    /// # Errors
    ///
    /// Propagates `CompactionError` variants (ThresholdNotReached, InsufficientHistory,
    /// ClientError, Serialization).
    async fn run_compaction(&mut self) -> Result<(), SessionError> {
        // Build a fresh snapshot for the compacted system prompt.
        let snapshot = self.snapshot_builder.build()?;
        let tokens_before = self.token_state.current_context_tokens;

        match &self.config.provider {
            Provider::Anthropic => {
                // Construct the Anthropic HTTP client using the session's API key and model.
                // Clone the http_client so the compaction call shares the same connection pool.
                let compaction_client = AnthropicCompactionClient {
                    api_key: self.config.api_key.clone(),
                    model_id: self.config.model_id.clone(),
                    http: self.http_client.clone(),
                };

                // Run the compaction pipeline — summarizes old history and rebuilds messages.
                let result = compact(
                    self.messages.clone(),
                    &snapshot,
                    &compaction_client,
                    &self.config.compaction,
                    tokens_before,
                )
                .await?;

                // Replace conversation history with the compacted version.
                self.messages = result.messages;
                // Reset token state — the next API call will give us fresh exact counts.
                self.token_state.reset();
                // Clear the read ledger — the model's mental model of file contents is stale.
                self.dispatcher.notify_compaction();

                // Notify the UI that compaction occurred.
                let _ = self
                    .event_tx
                    .send(SessionEvent::CompactionOccurred {
                        tokens_before,
                        tokens_after: result.tokens_after,
                    })
                    .await;
            }
            other => {
                // Compaction is not yet implemented for non-Anthropic providers.
                // Emit a warning and skip — the session continues without compacting.
                tracing::warn!(
                    "Context compaction not supported for provider {:?} — skipping",
                    other
                );
                let _ = self
                    .event_tx
                    .send(SessionEvent::Warning {
                        message: format!(
                            "Compaction not supported for provider {:?}",
                            self.config.provider
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
///
/// Produces an assistant message containing:
///   - A `Text` block if the model emitted any text.
///   - `ToolCall` blocks for each tool call the model requested.
///
/// The `with_stop` builder attaches the stop reason if one was emitted.
fn build_assistant_message(result: &StreamResult) -> ConversationMessage {
    let mut blocks: Vec<ContentBlock> = Vec::new();

    // Include text only if the model actually generated some.
    if !result.text.is_empty() {
        blocks.push(ContentBlock::Text(result.text.clone()));
    }

    // Append one ToolCall block per tool call, in emission order.
    for call in &result.tool_calls {
        blocks.push(ContentBlock::ToolCall(call.clone()));
    }

    // Build the base assistant message.
    let mut msg = ConversationMessage::assistant(blocks);

    // Attach the stop reason if the stream emitted one.
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
/// Returns `None` if the required fields are absent or cannot be parsed as u64.
fn extract_usage_record(
    raw: &serde_json::Value,
    config: &SessionConfig,
) -> Option<UsageRecord> {
    // Try Anthropic field names first, then fall back to OpenAI names.
    let input = raw
        .get("input_tokens")
        .or_else(|| raw.get("prompt_tokens"))
        .and_then(|v| v.as_u64())? as usize;

    let output = raw
        .get("output_tokens")
        .or_else(|| raw.get("completion_tokens"))
        .and_then(|v| v.as_u64())? as usize;

    // Anthropic prompt cache fields — optional, only present when caching is active.
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
        model: config.model_id.clone(),
        provider: format!("{:?}", config.provider),
    })
}

/// Generate a unique session ID using the current nanosecond timestamp in hex.
///
/// This is the same scheme used by `SnapshotBuilder::generate_session_id` so
/// session IDs from both sources have a consistent format.
fn generate_session_id() -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    format!("{nanos:x}")
}
