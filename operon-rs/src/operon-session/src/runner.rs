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
//   6. Record token usage + emit TokenUsageUpdated
//   7. Push assistant message into history
//   8. Persist turn to SQLite
//   9. If no tool calls → Done; break
//  10. Check for Cancel command
//  11. Dispatch tool calls sequentially → emit ToolDegraded if needed → loop back
//
// ── Three-directional model integration ──────────────────────────────────────
//
// At startup (new()), if config.project_dir is Some, the runner injects it into
// the PolicyConfig with owner-full-access permissions. This is Direction 3 — a
// project directory opened VS Code-style that is temporarily allowed for this
// session but not persisted to config.toml.

use reqwest::Client;
use tokio::sync::mpsc;

use operon_context_compaction::{compact, AnthropicCompactionClient};
use operon_context_normalize_messages::{ConversationMessage, ContentBlock, MessageRole};
use operon_context_normalize_tools::ToolContent;
use operon_context_sanitizer::sanitize;
use operon_context_snapshot::SnapshotBuilder;
use operon_context_token_tracker::{SessionTokenState, TokenBudget, UsageRecord};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::config::{DirectoryPolicy, PolicyConfig};
use operon_providers::Provider;
use operon_tools::dispatcher::Dispatcher;

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
    /// 1. Inject `project_dir` into `PolicyConfig` if present (Direction 3).
    /// 2. Generate unique session ID.
    /// 3. Build `SnapshotBuilder` (starts filesystem watcher).
    /// 4. Register tool groups on `Dispatcher`.
    /// 5. Open SQLite store if configured.
    /// 6. Emit `SessionStarted` — UI receives this immediately after construction.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if project_dir cannot be canonicalized, the snapshot
    /// builder fails, or the SQLite store cannot be opened.
    pub async fn new(
        mut config: SessionConfig,
        event_tx: mpsc::Sender<SessionEvent>,
        cmd_rx: mpsc::Receiver<SessionCommand>,
    ) -> Result<Self, SessionError> {
        // ── Direction 3: inject project_dir into policy ───────────────────────
        // If a project directory is open (VS Code-style), temporarily add it to
        // the policy with owner-full-access. This entry lives only for this session
        // and is never written to config.toml.
        if let Some(ref proj_dir) = config.project_dir {
            // Owner gets full access — they opened this project directory.
            // External gets zero access — project dirs are owner-only.
            let proj_policy = DirectoryPolicy::owner_full_access(proj_dir.clone());
            config.policy.directories.push(proj_policy);

            // validate() canonicalizes the new project_dir path.
            config.policy.validate()
                .map_err(|e| SessionError::Config(e.to_string()))?;
        }

        // Generate a unique session ID using nanosecond hex timestamp.
        let session_id = generate_session_id();

        // Build the snapshot builder — this also starts the filesystem watcher.
        let snapshot_config = config.snapshot_config(&session_id);
        let snapshot_builder = SnapshotBuilder::new(snapshot_config)?;

        // Initialize the dispatcher and register the "load_tools" meta-tool.
        let mut dispatcher = Dispatcher::new();
        dispatcher.register_load_tool();

        // Register tool groups based on the session configuration.
        for group in &config.tool_groups {
            match group.as_str() {
                "fs"    => dispatcher.register_fs_tools(),
                "shell" => dispatcher.register_shell_tools(),
                "web"   => dispatcher.register_web_tools(),
                "todo"  => dispatcher.register_todo_tools(),
                other   => tracing::warn!("Unknown tool group: {other}"),
            }
        }

        // Build the token budget from the provider config's context window size.
        let token_budget = TokenBudget::with_window(config.provider_config.context_window())
            .map_err(|e| SessionError::Stream(e.to_string()))?;

        // Open the SQLite store if a path was provided in the configuration.
        let store = if let Some(path) = &config.store_path {
            let s = SessionStore::open(path).await?;
            s.create_session(
                &session_id,
                &config.workspace_root.display().to_string(),
                config.provider_config.model_id(),
                &format!("{:?}", config.provider_config.provider),
            )
            .await?;
            Some(s)
        } else {
            None
        };

        // Emit SessionStarted — the UI now knows the session ID and can label panels.
        // This is the first event on the channel; it fires before any turn runs.
        let _ = event_tx
            .send(SessionEvent::SessionStarted {
                session_id: session_id.clone(),
            })
            .await;

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
    ///   5. Record token usage + emit TokenUsageUpdated
    ///   6. Push assistant message into history
    ///   7. Persist turn to SQLite
    ///   8. If no tool calls → emit Done + break
    ///   9. Check for Cancel command
    ///  10. Dispatch tool calls sequentially + emit ToolDegraded if needed → loop back
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
            if self.token_budget.should_compact(self.token_state.current_context_tokens) {
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
                        operon_context_compaction::CompactionError::ThresholdNotReached,
                    )) => {
                        tracing::warn!("Compaction triggered but threshold not reached — skipping");
                    }
                    Err(SessionError::Compaction(
                        operon_context_compaction::CompactionError::InsufficientHistory,
                    )) => {
                        let _ = self.event_tx
                            .send(SessionEvent::Warning {
                                message: "Context compaction skipped: insufficient history".to_string(),
                            })
                            .await;
                    }
                    Err(e) => {
                        self.lifecycle = LifecycleState::Failed;
                        return Err(e);
                    }
                }
            }

            // ── 2. Build snapshot + sanitize ─────────────────────────────────
            let snapshot = self.snapshot_builder.build()?;
            let clean_messages = sanitize(
                self.messages.clone(),
                &snapshot,
                self.config.role,
            )?;

            // ── 3. Collect tool definitions ──────────────────────────────────
            let tool_defs: Vec<_> = self.dispatcher.definitions().cloned().collect();

            // ── 4. Build request body ────────────────────────────────────────
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
            let endpoint = self.config.provider_config.effective_base_url().to_string();
            let api_key  = self.config.provider_config.credentials.api_key.expose().to_string();

            let stream_result = send_streaming(
                &self.http_client,
                &self.config.provider_config.provider,
                &endpoint,
                &api_key,
                body,
                &self.event_tx,
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
                            input_tokens:      record.input_tokens,
                            output_tokens:     record.output_tokens,
                            context_total:     self.token_state.current_context_tokens,
                            cache_read_tokens:  record.cache_read_tokens,
                            cache_write_tokens: record.cache_write_tokens,
                        })
                        .await;
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
            // Poll the command channel without blocking. If the user sent Cancel
            // between turns, finish cleanly now rather than dispatching more tool calls.
            if let Ok(SessionCommand::Cancel) = self.cmd_rx.try_recv() {
                tracing::info!("Session cancelled by user command");
                let _ = self.event_tx.send(SessionEvent::Done).await;
                self.lifecycle = LifecycleState::Done;
                break;
            }

            // ── 11. Dispatch tool calls sequentially ─────────────────────────
            // Tool calls are dispatched in the order the model emitted them.
            // Do NOT parallelize — order matters for read-ledger enforcement.
            let mut tool_results: Vec<ContentBlock> = Vec::new();

            for call in stream_result.tool_calls {
                // dispatch() returns DispatchOutcome so we can observe degradation.
                let outcome = self.dispatcher.dispatch(call).await;

                // If this is the FIRST malformed call for this tool, emit ToolDegraded
                // so the TUI can show a warning badge on the tool.
                if let Some(ref name) = outcome.newly_degraded {
                    let _ = self
                        .event_tx
                        .send(SessionEvent::ToolDegraded { name: name.clone() })
                        .await;
                }

                let result = outcome.result;

                let content_json = match &result.content {
                    ToolContent::Text(s) => s.clone(),
                    ToolContent::Json(v) => v.to_string(),
                };

                let _ = self
                    .event_tx
                    .send(SessionEvent::ToolCallResult {
                        call_id:      result.call_id.0.clone(),
                        name:         result.name.clone(),
                        is_error:     result.is_error,
                        content_json: content_json.clone(),
                    })
                    .await;

                tool_results.push(ContentBlock::ToolResult(result));
            }

            // Push all tool results as a single Tool-role message.
            self.messages.push(ConversationMessage {
                role:        MessageRole::Tool,
                content:     tool_results,
                stop_reason: None,
            });

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
    /// Includes the injected project_dir entry (if any). Useful for the TUI
    /// to display which directories are currently accessible.
    pub fn policy(&self) -> &PolicyConfig {
        &self.config.policy
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Private helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Run context compaction: summarize old history and rebuild the message array.
    async fn run_compaction(&mut self) -> Result<(), SessionError> {
        let snapshot      = self.snapshot_builder.build()?;
        let tokens_before = self.token_state.current_context_tokens;

        match &self.config.provider_config.provider {
            Provider::Anthropic => {
                let compaction_client = AnthropicCompactionClient {
                    api_key:  self.config.provider_config.credentials.api_key.expose().to_string(),
                    model_id: self.config.provider_config.model_id().to_string(),
                    http:     self.http_client.clone(),
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
        input_tokens:       input,
        output_tokens:      output,
        cache_read_tokens:  cache_read,
        cache_write_tokens: cache_write,
        model:              model_id.to_string(),
        provider:           provider_name.to_string(),
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
