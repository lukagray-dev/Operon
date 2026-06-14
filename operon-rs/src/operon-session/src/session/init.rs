// init.rs — Handles the initialization of a SessionRunner.
//
// Hey friend! This file implements the constructor logic for the session runner.
// It generates a session ID, initializes the SQLite database if configured,
// sets up the snapshot builder (which starts watching files), registers all
// tool groups, and returns the constructed runner instance.

use std::collections::VecDeque;
use reqwest::Client;
use tokio::sync::mpsc;

use operon_context::{SessionTokenState, SnapshotBuilder, TokenBudget};
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::PolicyResolver;
use operon_tools::dispatcher::Dispatcher;

use crate::config::SessionConfig;
use crate::error::SessionError;
use crate::lifecycle::LifecycleState;
use crate::store::SessionStore;
use crate::runner::SessionRunner;
use super::events::context_usage_event;

/// Generate a unique session ID using the current nanosecond timestamp in hex.
pub fn generate_session_id() -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    format!("{nanos:x}")
}

/// The full initialization logic of SessionRunner, moved out of runner.rs.
pub async fn new_runner(
    config: SessionConfig,
    event_tx: mpsc::Sender<SessionEvent>,
    cmd_rx: mpsc::Receiver<SessionCommand>,
) -> Result<SessionRunner, SessionError> {
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

    Ok(SessionRunner {
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
