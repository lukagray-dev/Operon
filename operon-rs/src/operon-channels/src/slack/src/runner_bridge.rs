// runner_bridge.rs — Operon SessionRunner bridge for Slack channel.
//
// Hey friend! This file bridges Slack incoming messages to `operon_session::SessionRunner`.
//
// Flow per inbound turn:
//   1. Check if first-time user -> auto-send onboarding documentation over Slack.
//   2. Provision user workspace (`~/.operon/sessions/slack/<user_id>/`) & system instructions.
//   3. Compute JSON session store path (`~/.operon/sessions/slack/<user_id>/<session_id>.json`).
//   4. Construct `SessionConfig` with `Role::Owner` or `Role::External`.
//   5. Open SessionStore, load prior history if session exists, compute turn_index / last_token_count.
//   6. Instantiate `SessionRunner` and call `set_history()` if resuming an existing session.
//   7. Execute turn and listen to `SessionEvent` stream:
//      - `ApprovalRequired`: send permission prompt instruction to desktop GUI.
//      - `ToolCallStart`: send tool progress update (e.g. `⚡ Executing: web_search`).
//      - `TextDelta` / `Done`: send final formatted response payload back to Slack.

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use operon_config::AppConfig;
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::CallerRole;
use operon_session::store::SessionStore;
use operon_session::{SessionConfig, SessionRunner};

use crate::error::SlackError;
use crate::outbound::SlackOutboundMessage;
use crate::router::SlackRouter;
use crate::types::{SlackChannelId, UserId};
use crate::workspace::SlackWorkspaceManager;

/// Hook signature for external consumers (e.g. GUI) listening to live channel session events and commands.
pub type SessionEventHook =
    Arc<dyn Fn(&str, &SessionEvent, &mpsc::Sender<SessionCommand>) + Send + Sync>;

/// Bridge that drives `SessionRunner` for a specific Slack user and sends output over the Slack outbound channel.
pub struct SessionRunnerBridge {
    app_config: AppConfig,
    workspace_manager: SlackWorkspaceManager,
    outbound_tx: mpsc::Sender<SlackOutboundMessage>,
    router: Option<Arc<SlackRouter>>,
    event_hook: Option<SessionEventHook>,
}

impl SessionRunnerBridge {
    /// Creates a new `SessionRunnerBridge` with loaded `AppConfig` and outbound message channel sender.
    pub fn new(
        app_config: AppConfig,
        workspace_manager: SlackWorkspaceManager,
        outbound_tx: mpsc::Sender<SlackOutboundMessage>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: None,
            event_hook: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `SlackRouter` to support turn cancellation on `/new`.
    pub fn with_router(
        app_config: AppConfig,
        workspace_manager: SlackWorkspaceManager,
        outbound_tx: mpsc::Sender<SlackOutboundMessage>,
        router: Arc<SlackRouter>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: Some(router),
            event_hook: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `SlackRouter` and an external `SessionEventHook`.
    pub fn with_router_and_hook(
        app_config: AppConfig,
        workspace_manager: SlackWorkspaceManager,
        outbound_tx: mpsc::Sender<SlackOutboundMessage>,
        router: Arc<SlackRouter>,
        event_hook: Option<SessionEventHook>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: Some(router),
            event_hook,
        }
    }

    /// Auto-sends first-time onboarding documentation message over Slack.
    pub async fn send_onboarding(
        &self,
        channel_id: &SlackChannelId,
        thread_ts: Option<String>,
    ) -> Result<(), SlackError> {
        let text = "👋 *Welcome to Operon!*\n\n\
             I am your autonomous AI assistant running locally on Operon.\n\n\
             💡 *Shortcuts & Tips:*\n\
             • Send `/new` anytime to start a fresh, clean session.\n\
             • You can ask questions, run web searches, analyze files, and manage tasks.\n\n\
             _Starting your session now..._";
        let msg = SlackOutboundMessage::new_threaded(channel_id.as_str(), text, thread_ts);
        let _ = self.outbound_tx.send(msg).await;
        Ok(())
    }

    /// Process a turn for a user message over Slack.
    pub async fn process_turn(
        &self,
        user_id: &UserId,
        channel_id: &SlackChannelId,
        session_id: &str,
        thread_ts: Option<String>,
        role: CallerRole,
        user_message: String,
        is_first_time: bool,
    ) -> Result<(), SlackError> {
        // Send onboarding doc on first message from this user
        if is_first_time {
            let _ = self.send_onboarding(channel_id, thread_ts.clone()).await;
        }

        // 1. Provision user workspace & role-specific channel instructions
        let is_owner = matches!(role, CallerRole::Owner);
        let workspace_root = self
            .workspace_manager
            .provision_workspace(user_id, is_owner)?;

        let channel_instructions = if is_owner {
            crate::workspace::generate_owner_channel_instructions(user_id)
        } else {
            crate::workspace::generate_external_channel_instructions(user_id)
        };

        // 2. Compute JSON session store path
        let store_path = self
            .workspace_manager
            .session_file_path_for(user_id, session_id);

        // Map CallerRole to context Role
        let context_role = match role {
            CallerRole::Owner => operon_context::Role::Owner,
            CallerRole::External => operon_context::Role::External,
        };

        // 3. Construct SessionConfig
        let session_config = SessionConfig {
            provider_config: self.app_config.provider.clone(),
            policy: self.app_config.policy.clone(),
            project_dir: None,
            workspace_root: workspace_root.clone(),
            role: context_role,
            tool_groups: SessionConfig::default_tool_groups(),
            compaction: operon_context::CompactionConfig::with_context_window(
                self.app_config.provider.context_window(),
            ),
            store_path: Some(store_path.clone()),
            channel_instructions: Some(channel_instructions),
        };

        // ── 4. Session history loading ──────────────────────────────────────────
        let is_new_session = is_first_time && !store_path.exists();

        // Open the SessionStore at store_path (creates parent dirs if needed).
        let store = SessionStore::open(&store_path)
            .await
            .map_err(|e| SlackError::Session(e.to_string()))?;

        if is_new_session {
            store
                .create_session(
                    session_id,
                    &workspace_root.to_string_lossy(),
                    session_config.provider_config.model_id(),
                    &format!("{:?}", session_config.provider_config.provider),
                )
                .await
                .map_err(|e| SlackError::Session(e.to_string()))?;
        }

        let history = store
            .load_full_history(session_id)
            .await
            .map_err(|e| SlackError::Session(e.to_string()))?;

        let history_turns = store
            .load_turns(session_id)
            .await
            .map_err(|e| SlackError::Session(e.to_string()))?;

        let turn_index = history_turns.len();
        let last_token_count = store
            .get_last_token_count(session_id)
            .await
            .map_err(|e| SlackError::Session(e.to_string()))?;

        info!(
            "Slack session {} for user {}: is_new={}, history_turns={}, turn_index={}, last_token_count={:?}",
            session_id, user_id, is_new_session, history_turns.len(), turn_index, last_token_count
        );

        // ── 5. Create mpsc channels for SessionEvent and SessionCommand ─────
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(100);
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(10);

        // Wire cmd_tx to the router so /new can cancel in-flight turns!
        if let Some(ref router) = self.router {
            router
                .register_cmd_tx(user_id, session_id, cmd_tx.clone())
                .await;
        }

        // ── 6. Instantiate SessionRunner and restore history ────────────────
        let mut runner = SessionRunner::new(session_config, event_tx, cmd_rx)
            .await
            .map_err(|e| SlackError::Session(e.to_string()))?;

        if !history.is_empty() {
            runner.set_history(history, turn_index, last_token_count);
        }

        // ── 7. Spawn runner task ────────────────────────────────────────────
        let runner_handle =
            tokio::spawn(async move { runner.run(user_message, vec![], vec![]).await });

        // ── 8. Event consumer loop — forward tool progress & final text ─────
        forward_session_events_to_outbound(
            channel_id,
            session_id,
            thread_ts.clone(),
            &cmd_tx,
            self.event_hook.as_ref(),
            &self.outbound_tx,
            &mut event_rx,
        )
        .await;

        // Unregister cmd_tx from router upon turn completion
        if let Some(ref router) = self.router {
            router.unregister_cmd_tx(user_id, session_id).await;
        }

        // Wait for runner task to finish cleanly
        if let Err(e) = runner_handle.await {
            info!(
                "Runner handle ended (may have been aborted/cancelled): {}",
                e
            );
        }

        Ok(())
    }
}

async fn forward_session_events_to_outbound(
    channel_id: &SlackChannelId,
    session_id: &str,
    thread_ts: Option<String>,
    cmd_tx: &mpsc::Sender<SessionCommand>,
    event_hook: Option<&SessionEventHook>,
    outbound_tx: &mpsc::Sender<SlackOutboundMessage>,
    event_rx: &mut mpsc::Receiver<SessionEvent>,
) {
    let mut accumulated_text = String::new();
    let mut terminal_event_seen = false;

    while let Some(event) = event_rx.recv().await {
        if let Some(hook) = event_hook {
            hook(session_id, &event, cmd_tx);
        }

        match event {
            SessionEvent::ApprovalRequired { ref tool, .. } => {
                let msg = format!("⚠️ *Permission Required:* Operon wants to run `{}`. Please allow or deny in the Operon Desktop GUI.", tool);
                let out = SlackOutboundMessage::new_threaded(
                    channel_id.as_str(),
                    &msg,
                    thread_ts.clone(),
                );
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::ToolCallStart { name, .. } => {
                let progress_msg = format!("⚡ *Executing:* `{}`", name);
                let out = SlackOutboundMessage::new_threaded(
                    channel_id.as_str(),
                    &progress_msg,
                    thread_ts.clone(),
                );
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::TextDelta { text } => {
                accumulated_text.push_str(&text);
            }
            SessionEvent::Done => {
                send_final_text(channel_id, thread_ts.clone(), outbound_tx, &accumulated_text).await;
                terminal_event_seen = true;
                break;
            }
            SessionEvent::Error { message } => {
                error!("SessionRunner error for Slack channel {}: {}", channel_id, message);
                let err_out = SlackOutboundMessage::new_threaded(
                    channel_id.as_str(),
                    &format!("❌ *Error:* {}", message),
                    thread_ts.clone(),
                );
                let _ = outbound_tx.send(err_out).await;
                terminal_event_seen = true;
                break;
            }
            other => {
                tracing::debug!(
                    ?other,
                    "SessionEvent variant intentionally ignored for Slack forwarding"
                );
            }
        }
    }

    if !terminal_event_seen {
        send_final_text(channel_id, thread_ts, outbound_tx, &accumulated_text).await;
    }
}

async fn send_final_text(
    channel_id: &SlackChannelId,
    thread_ts: Option<String>,
    outbound_tx: &mpsc::Sender<SlackOutboundMessage>,
    accumulated_text: &str,
) {
    let trimmed = accumulated_text.trim();
    if !trimmed.is_empty() {
        let msg = SlackOutboundMessage::new_threaded(channel_id.as_str(), trimmed, thread_ts);
        let _ = outbound_tx.send(msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_events::SessionEvent;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn forwards_final_text_as_soon_as_done_arrives() {
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<SlackOutboundMessage>(10);
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(10);
        let (cmd_tx, _cmd_rx) = mpsc::channel::<SessionCommand>(10);

        let ch = SlackChannelId::new("C123456");

        let forward_handle = tokio::spawn(async move {
            forward_session_events_to_outbound(
                &ch,
                "sl-test",
                None,
                &cmd_tx,
                None,
                &outbound_tx,
                &mut event_rx,
            )
            .await;
        });

        event_tx
            .send(SessionEvent::TextDelta {
                text: "Hello from ".to_string(),
            })
            .await
            .unwrap();
        event_tx
            .send(SessionEvent::TextDelta {
                text: "Slack!".to_string(),
            })
            .await
            .unwrap();
        event_tx.send(SessionEvent::Done).await.unwrap();

        let received = outbound_rx.recv().await.expect("Expected final message");
        assert_eq!(received.text, "Hello from Slack!");

        forward_handle.await.unwrap();
    }
}

