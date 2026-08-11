// runner/mod.rs — The Operon agent loop.
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

use reqwest::Client;
use tokio::sync::mpsc;

use operon_config::PolicyConfig;
use operon_context::{
    ContentBlock, ConversationMessage, SessionTokenState, SnapshotBuilder, TokenBudget,
    ToolContent,
};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::PolicyResolver;
use operon_tools::dispatcher::Dispatcher;

use crate::config::SessionConfig;
use crate::error::SessionError;
use crate::lifecycle::LifecycleState;
use crate::store::SessionStore;

// ── Submodule declarations ───────────────────────────────────────────────────
// Each submodule contains a logically separable concern of SessionRunner.
// Together they form the complete runner implementation.

mod commands;
mod compaction;
mod loop_impl;
mod message_build;
mod policy_path;
mod tool_dispatch;

// ── Re-imports ───────────────────────────────────────────────────────────────
// Bring submodule items into this scope for use in mod.rs code (e.g. new()).
pub use message_build::build_user_message;
use message_build::{context_usage_event, generate_session_id};

// Re-import free functions so runner_tests.rs (`use super::*`) can access them.
// These are only referenced from the test module, not from production code in mod.rs.
#[cfg(test)]
use commands::command_matches;
#[cfg(test)]
use message_build::{
    build_assistant_message, opaque_permission_denied_result, tool_result_content_json,
};
#[cfg(test)]
use policy_path::policy_path_for_call;

// Re-import types that runner_tests.rs references via `use super::*`.
// These types were in scope when runner was a single file; now they must
// be explicitly re-imported for the test module.
#[cfg(test)]
use crate::http::StreamResult;
#[cfg(test)]
use operon_context::{MessageRole, Role};

// ─────────────────────────────────────────────────────────────────────────────
// SessionRunner
// ─────────────────────────────────────────────────────────────────────────────

/// The Operon agent loop — owns all session state and drives the agentic cycle.
///
/// # Thread safety
///
/// `SessionRunner` is `Send` but not `Sync` (held in a single async task).
/// Wrap in `Arc<Mutex<..>>` only if you need cross-task access.
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
                        // Note: load_tools output format must stay JSON (or set_history must be updated in lockstep).
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

    /// Resume a paused session with a new user message and attachments.
    pub async fn resume(
        &mut self,
        user_message: String,
        image_blocks: Vec<ContentBlock>,
        file_paths: Vec<std::path::PathBuf>,
    ) -> Result<(), SessionError> {
        self.run(user_message, image_blocks, file_paths).await
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

// ── Test module wiring ───────────────────────────────────────────────────────
// The test file lives at src/runner_tests.rs (one directory up from runner/).
// The #[path] attribute uses a relative path from this file's location.

#[cfg(test)]
#[path = "../runner_tests.rs"]
mod runner_tests;
