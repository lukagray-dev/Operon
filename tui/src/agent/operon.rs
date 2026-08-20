// operon.rs — Real AgentBridge implementation connecting TUI to operon-rs SessionRunner.
//
// Hey friend! This file connects the TUI chat interface to the real AI agent loop in operon-rs.
// When the user submits a message:
// 1. We load the current configuration (active AI provider, model, API keys, policy rules).
// 2. We resolve or create a persistent session store file in `~/.operon/sessions/<id>.json`.
// 3. We construct a `SessionRunner` with outbound event channels and inbound command channels.
// 4. If resuming an existing session, we load the prior history into the runner.
// 5. We stream text and thinking deltas into the TUI's active message history.
// 6. We hold an active `cmd_tx` handle so the user can hit `Esc` to cleanly cancel the turn at any time.

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use anyhow::Result;

use crate::events::action::Action;
use super::AgentBridge;

/// Production agent implementation driving `operon_rs::session::SessionRunner`.
pub struct OperonAgent {
    /// Inbound command sender for the currently running session turn (if active).
    /// Used by `cancel()` to interrupt prompt execution cleanly without killing the app.
    active_cmd_tx: Arc<Mutex<Option<mpsc::Sender<operon_rs::SessionCommand>>>>,

    /// Unique session identifier for the active conversation.
    active_session_id: Arc<Mutex<Option<String>>>,
}

impl OperonAgent {
    /// Creates a new `OperonAgent` instance with no active command handle.
    pub fn new() -> Self {
        Self {
            active_cmd_tx: Arc::new(Mutex::new(None)),
            active_session_id: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for OperonAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentBridge for OperonAgent {
    /// Executes a user prompt through the `SessionRunner` and streams output events to `action_tx`.
    async fn execute_prompt(&self, prompt: String, action_tx: mpsc::Sender<Action>) -> Result<()> {
        // 1. Load active application configuration from ~/.operon/config.toml
        let app_config = match operon_rs::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                let _ = action_tx
                    .send(Action::AgentError(format!(
                        "Configuration error: {}. Please configure your model in the Models screen (press / -> Models).",
                        e
                    )))
                    .await;
                let _ = action_tx.send(Action::AgentDone).await;
                return Ok(());
            }
        };

        // 2. Resolve Operon runtime filesystem paths (workspace directory, persistent database)
        let paths = match operon_rs::OperonPaths::resolve() {
            Ok(p) => p,
            Err(e) => {
                let _ = action_tx
                    .send(Action::AgentError(format!("Path resolution failed: {}", e)))
                    .await;
                let _ = action_tx.send(Action::AgentDone).await;
                return Ok(());
            }
        };

        // 3. Create communication channels for session events and interactive commands
        let (event_tx, mut event_rx) = mpsc::channel::<operon_rs::SessionEvent>(100);
        let (cmd_tx, cmd_rx) = mpsc::channel::<operon_rs::SessionCommand>(20);

        // Store active cmd_tx so the user can trigger cancellation from the UI
        {
            let mut lock = self.active_cmd_tx.lock().await;
            *lock = Some(cmd_tx.clone());
        }

        // 4. Resolve session ID and persistent store path on disk
        let session_id = {
            let mut lock = self.active_session_id.lock().await;
            if let Some(id) = lock.as_ref() {
                id.clone()
            } else {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let new_id = format!("session-{}", now);
                *lock = Some(new_id.clone());
                new_id
            }
        };

        let store_path = paths.sessions_dir.join(format!("{}.json", session_id));
        let is_existing_session = store_path.exists();

        // 5. Build session runtime configuration using standard operon-rs defaults
        let session_config = operon_rs::session::SessionConfig {
            provider_config: app_config.provider.clone(),
            policy: app_config.policy.clone(),
            project_dir: None,
            workspace_root: paths.workspace_dir.clone(),
            role: operon_rs::context::Role::Owner,
            tool_groups: operon_rs::session::SessionConfig::default_tool_groups(),
            compaction: operon_rs::context::CompactionConfig::with_context_window(app_config.provider.context_window()),
            store_path: Some(store_path.clone()),
            channel_instructions: None,
        };

        // 6. Instantiate the cold SessionRunner
        let mut runner = match operon_rs::session::SessionRunner::new(session_config, event_tx, cmd_rx).await {
            Ok(r) => r,
            Err(e) => {
                // Clear active command sender
                {
                    let mut lock = self.active_cmd_tx.lock().await;
                    *lock = None;
                }
                let _ = action_tx
                    .send(Action::AgentError(format!("Failed to start agent session: {}", e)))
                    .await;
                let _ = action_tx.send(Action::AgentDone).await;
                return Ok(());
            }
        };

        // If resuming an existing session, load previous turn history into the runner
        if is_existing_session {
            if let Ok(store) = operon_rs::session::store::SessionStore::open(&store_path).await {
                if let Ok(history) = store.load_full_history(&session_id).await {
                    let turns = store.load_turns(&session_id).await.unwrap_or_default();
                    let last_tokens = store.get_last_token_count(&session_id).await.ok().flatten();
                    if !history.is_empty() {
                        runner.set_history(history, turns.len(), last_tokens);
                    }
                }
            }
        }

        let action_tx_events = action_tx.clone();
        let active_cmd_tx_holder = Arc::clone(&self.active_cmd_tx);

        // 7. Spawn background event forwarder task reading from SessionRunner's event channel
        let event_forwarder = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    operon_rs::SessionEvent::TextDelta { text } => {
                        let _ = action_tx_events.send(Action::AgentTextDelta(text)).await;
                    }
                    operon_rs::SessionEvent::ThinkingDelta { text } => {
                        let _ = action_tx_events.send(Action::AgentThinkingDelta(text)).await;
                    }
                    operon_rs::SessionEvent::ContextUsageUpdated {
                        current_context_tokens,
                        context_window,
                        ..
                    } => {
                        let _ = action_tx_events
                            .send(Action::AgentContextUpdate {
                                current_tokens: current_context_tokens,
                                total_window: context_window,
                            })
                            .await;
                    }
                    operon_rs::SessionEvent::TokenUsageUpdated { context_total, .. } => {
                        let _ = action_tx_events
                            .send(Action::AgentContextUpdate {
                                current_tokens: context_total,
                                total_window: 0,
                            })
                            .await;
                    }
                    operon_rs::SessionEvent::Done => {
                        let _ = action_tx_events.send(Action::AgentDone).await;
                    }
                    operon_rs::SessionEvent::Error { message } => {
                        let _ = action_tx_events.send(Action::AgentError(message)).await;
                    }
                    operon_rs::SessionEvent::Warning { message } => {
                        let _ = action_tx_events
                            .send(Action::AgentTextDelta(format!("\n[Warning: {}]\n", message)))
                            .await;
                    }
                    _ => {}
                }
            }
        });

        // 8. Execute the prompt in the SessionRunner loop
        let run_result = runner.run(prompt, Vec::new(), Vec::new()).await;
        if let Err(e) = run_result {
            let _ = action_tx
                .send(Action::AgentError(format!("Agent loop error: {}", e)))
                .await;
        }

        // Explicitly drop runner to close event_tx and finish event forwarder
        drop(runner);
        let _ = event_forwarder.await;

        // Clear active command sender
        {
            let mut lock = active_cmd_tx_holder.lock().await;
            *lock = None;
        }

        // Ensure AgentDone signal is dispatched to finalize UI state
        let _ = action_tx.send(Action::AgentDone).await;

        Ok(())
    }

    /// Cancels the currently active prompt execution turn.
    async fn cancel(&self) -> Result<()> {
        let lock = self.active_cmd_tx.lock().await;
        if let Some(cmd_tx) = lock.as_ref() {
            let _ = cmd_tx.send(operon_rs::SessionCommand::Cancel).await;
        }
        Ok(())
    }

    /// Sets or clears the active session ID.
    fn set_session_id(&mut self, session_id: Option<String>) {
        if let Ok(mut lock) = self.active_session_id.try_lock() {
            *lock = session_id;
        }
    }

    /// Returns the currently active session ID.
    fn session_id(&self) -> Option<String> {
        self.active_session_id.try_lock().ok().and_then(|lock| lock.clone())
    }
}
