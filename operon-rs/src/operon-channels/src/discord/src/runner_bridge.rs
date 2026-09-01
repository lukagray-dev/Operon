// runner_bridge.rs — Operon SessionRunner bridge for Discord channel.
//
// Hey friend! This file bridges Discord incoming messages to `operon_session::SessionRunner`.
//
// Flow per inbound turn:
//   1. Check if first-time user -> auto-send onboarding documentation over Discord.
//   2. Provision user workspace (`~/.operon/sessions/discord/<user_id>/`) & system instructions.
//   3. Compute JSON session store path (`~/.operon/sessions/discord/<user_id>/<session_id>.json`).
//   4. Construct `SessionConfig` with `Role::Owner` or `Role::External`.
//   5. Open SessionStore, load prior history if session exists, compute turn_index / last_token_count.
//   6. Instantiate `SessionRunner` and call `set_history()` if resuming an existing session.
//   7. Execute turn and listen to `SessionEvent` stream:
//      - `ApprovalRequired`: send permission prompt instruction to desktop GUI.
//      - `ToolCallStart`: send tool progress update (e.g. `⚡ Executing: web_search`).
//      - `TextDelta` / `Done`: send final formatted response payload back to Discord.

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use operon_config::AppConfig;
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::CallerRole;
use operon_session::store::SessionStore;
use operon_session::{SessionConfig, SessionRunner};

use crate::error::DiscordError;
use crate::outbound::DiscordOutboundMessage;
use crate::router::DiscordRouter;
use crate::types::{DiscordChannelId, UserId};
use crate::workspace::DiscordWorkspaceManager;

/// Hook signature for external consumers (e.g. GUI) listening to live channel session events and commands.
pub type SessionEventHook =
    Arc<dyn Fn(&str, &SessionEvent, &mpsc::Sender<SessionCommand>) + Send + Sync>;

/// Bridge that drives `SessionRunner` for a specific Discord user and sends output over the Discord outbound channel.
pub struct SessionRunnerBridge {
    app_config: AppConfig,
    workspace_manager: DiscordWorkspaceManager,
    outbound_tx: mpsc::Sender<DiscordOutboundMessage>,
    router: Option<Arc<DiscordRouter>>,
    event_hook: Option<SessionEventHook>,
}

impl SessionRunnerBridge {
    /// Creates a new `SessionRunnerBridge` with loaded `AppConfig` and outbound message channel sender.
    pub fn new(
        app_config: AppConfig,
        workspace_manager: DiscordWorkspaceManager,
        outbound_tx: mpsc::Sender<DiscordOutboundMessage>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: None,
            event_hook: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `DiscordRouter` to support turn cancellation on `/new`.
    pub fn with_router(
        app_config: AppConfig,
        workspace_manager: DiscordWorkspaceManager,
        outbound_tx: mpsc::Sender<DiscordOutboundMessage>,
        router: Arc<DiscordRouter>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: Some(router),
            event_hook: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `DiscordRouter` and an external `SessionEventHook`.
    pub fn with_router_and_hook(
        app_config: AppConfig,
        workspace_manager: DiscordWorkspaceManager,
        outbound_tx: mpsc::Sender<DiscordOutboundMessage>,
        router: Arc<DiscordRouter>,
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

    /// Auto-sends first-time onboarding documentation message over Discord.
    pub async fn send_onboarding(
        &self,
        channel_id: &DiscordChannelId,
    ) -> Result<(), DiscordError> {
        let text = "👋 **Welcome to Operon!**\n\n\
             I am your autonomous AI assistant running locally on Operon.\n\n\
             💡 **Shortcuts & Tips:**\n\
             • Send `/new` anytime to start a fresh, clean session.\n\
             • You can ask questions, run web searches, analyze files, and manage tasks.\n\n\
             _Starting your session now..._";
        let msg = DiscordOutboundMessage::new(channel_id.as_str(), text);
        let _ = self.outbound_tx.send(msg).await;
        Ok(())
    }

    /// Process a turn for a user message over Discord.
    pub async fn process_turn(
        &self,
        user_id: &UserId,
        channel_id: &DiscordChannelId,
        session_id: &str,
        role: CallerRole,
        user_message: String,
        is_first_time: bool,
    ) -> Result<(), DiscordError> {
        // Send onboarding doc on first message from this user
        if is_first_time {
            let _ = self.send_onboarding(channel_id).await;
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
            .map_err(|e| DiscordError::Session(e.to_string()))?;

        if is_new_session {
            store
                .create_session(
                    session_id,
                    &workspace_root.to_string_lossy(),
                    session_config.provider_config.model_id(),
                    &format!("{:?}", session_config.provider_config.provider),
                )
                .await
                .map_err(|e| DiscordError::Session(e.to_string()))?;
        }

        let history = store
            .load_full_history(session_id)
            .await
            .map_err(|e| DiscordError::Session(e.to_string()))?;

        let history_turns = store
            .load_turns(session_id)
            .await
            .map_err(|e| DiscordError::Session(e.to_string()))?;

        let turn_index = history_turns.len();
        let last_token_count = store
            .get_last_token_count(session_id)
            .await
            .map_err(|e| DiscordError::Session(e.to_string()))?;

        info!(
            "Discord session {} for user {}: is_new={}, history_turns={}, turn_index={}, last_token_count={:?}",
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
            .map_err(|e| DiscordError::Session(e.to_string()))?;

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
    channel_id: &DiscordChannelId,
    session_id: &str,
    cmd_tx: &mpsc::Sender<SessionCommand>,
    event_hook: Option<&SessionEventHook>,
    outbound_tx: &mpsc::Sender<DiscordOutboundMessage>,
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
                let msg = format!("⚠️ **Permission Required:** Operon wants to run `{}`. Please allow or deny in the Operon Desktop GUI.", tool);
                let out = DiscordOutboundMessage::new(channel_id.as_str(), &msg);
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::ToolCallStart { name, .. } => {
                let progress_msg = format!("⚡ **Executing:** `{}`", name);
                let out = DiscordOutboundMessage::new(channel_id.as_str(), &progress_msg);
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::TextDelta { text } => {
                accumulated_text.push_str(&text);
            }
            SessionEvent::Done => {
                send_final_text(channel_id, outbound_tx, &accumulated_text).await;
                terminal_event_seen = true;
                break;
            }
            SessionEvent::Error { message } => {
                error!("SessionRunner error for Discord channel {}: {}", channel_id, message);
                let err_out =
                    DiscordOutboundMessage::new(channel_id.as_str(), &format!("❌ **Error:** {}", message));
                let _ = outbound_tx.send(err_out).await;
                terminal_event_seen = true;
                break;
            }
            other => {
                tracing::debug!(
                    ?other,
                    "SessionEvent variant intentionally ignored for Discord forwarding"
                );
            }
        }
    }

    if !terminal_event_seen {
        send_final_text(channel_id, outbound_tx, &accumulated_text).await;
    }
}

async fn send_final_text(
    channel_id: &DiscordChannelId,
    outbound_tx: &mpsc::Sender<DiscordOutboundMessage>,
    accumulated_text: &str,
) {
    let trimmed = accumulated_text.trim();
    if !trimmed.is_empty() {
        let final_msg = DiscordOutboundMessage::new(channel_id.as_str(), trimmed);
        let _ = outbound_tx.send(final_msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn forwards_final_text_as_soon_as_done_arrives() {
        let channel = DiscordChannelId::new("123456789012345678");
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<DiscordOutboundMessage>(8);
        let held_sender = event_tx.clone();

        event_tx
            .send(SessionEvent::TextDelta {
                text: "Hello ".to_string(),
            })
            .await
            .unwrap();
        event_tx
            .send(SessionEvent::TextDelta {
                text: "from Discord Operon".to_string(),
            })
            .await
            .unwrap();
        event_tx.send(SessionEvent::Done).await.unwrap();

        let (cmd_tx, _cmd_rx) = mpsc::channel::<SessionCommand>(1);
        tokio::time::timeout(
            Duration::from_secs(1),
            forward_session_events_to_outbound(
                &channel,
                "test-session",
                &cmd_tx,
                None,
                &outbound_tx,
                &mut event_rx,
            ),
        )
        .await
        .expect("Done must release the Discord event forwarder");

        let out = outbound_rx.recv().await.unwrap();
        assert_eq!(out.channel_id, "123456789012345678");
        assert_eq!(out.text, "Hello from Discord Operon");
        assert!(outbound_rx.try_recv().is_err());
        drop(held_sender);
    }
}

