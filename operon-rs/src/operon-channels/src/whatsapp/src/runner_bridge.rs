// runner_bridge.rs — Operon SessionRunner bridge for WhatsApp channel.
//
// Hey friend! This file bridges WhatsApp incoming messages to `operon_session::SessionRunner`.
//
// Flow per inbound turn:
//   1. Check if first-time user -> auto-send onboarding documentation over WhatsApp.
//   2. Provision contact workspace (`~/.operon/channels/whatsapp/workspace/<phone>/`) & `AGENTS.md`.
//   3. Compute JSON session store path (`~/.operon/sessions/whatsapp/<phone>/<session_id>.json`).
//   4. Construct `SessionConfig` with `Role::Owner` or `Role::External`.
//   5. Open SessionStore, load prior history if session file exists, compute turn_index / last_token_count.
//   6. Instantiate `SessionRunner` and call `set_history()` if resuming an existing session.
//   7. Execute turn and listen to `SessionEvent` stream:
//      - `ToolCallStart`: send tool progress update (e.g. `⚡ Running web_search...`).
//      - `TextDelta` / `Done`: send final formatted response payload back over WhatsApp socket.
//
// History loading mirrors gui/src/executor/session.rs::start_agent_session exactly:
//   - SessionStore::open(store_path)
//   - store.load_turns(session_id) -> history_turns
//   - turn_index = history_turns.len()
//   - last_token_count = store.get_last_token_count(session_id)
//   - runner.set_history(last_turn_messages, turn_index, last_token_count) if history non-empty
//
// Persistence is automatic — SessionRunner internally calls store.save_turn() after each
// assistant message in loop_impl.rs step 8. No extra append_turn() call needed here.

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use operon_config::AppConfig;
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::CallerRole;
use operon_session::store::SessionStore;
use operon_session::{SessionConfig, SessionRunner};

use crate::error::WhatsAppError;
use crate::outbound::OutboundMessage;
use crate::router::WhatsAppRouter;
use crate::types::ContactId;
use crate::workspace::WhatsAppWorkspaceManager;

/// Hook signature for external consumers (e.g. GUI) listening to live channel session events and commands.
pub type SessionEventHook = Arc<dyn Fn(&str, &SessionEvent, &mpsc::Sender<SessionCommand>) + Send + Sync>;

/// Bridge that drives `SessionRunner` for a specific contact and sends output over WhatsApp outbound channel.
pub struct SessionRunnerBridge {
    app_config: AppConfig,
    workspace_manager: WhatsAppWorkspaceManager,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    router: Option<Arc<WhatsAppRouter>>,
    event_hook: Option<SessionEventHook>,
}

impl SessionRunnerBridge {
    /// Creates a new `SessionRunnerBridge` with loaded `AppConfig` and outbound message channel sender.
    pub fn new(
        app_config: AppConfig,
        workspace_manager: WhatsAppWorkspaceManager,
        outbound_tx: mpsc::Sender<OutboundMessage>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: None,
            event_hook: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `WhatsAppRouter` to support turn cancellation on `/new`.
    pub fn with_router(
        app_config: AppConfig,
        workspace_manager: WhatsAppWorkspaceManager,
        outbound_tx: mpsc::Sender<OutboundMessage>,
        router: Arc<WhatsAppRouter>,
    ) -> Self {
        Self {
            app_config,
            workspace_manager,
            outbound_tx,
            router: Some(router),
            event_hook: None,
        }
    }

    /// Creates a new `SessionRunnerBridge` wired with `WhatsAppRouter` and an external `SessionEventHook`.
    pub fn with_router_and_hook(
        app_config: AppConfig,
        workspace_manager: WhatsAppWorkspaceManager,
        outbound_tx: mpsc::Sender<OutboundMessage>,
        router: Arc<WhatsAppRouter>,
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

    /// Auto-sends first-time onboarding documentation message over WhatsApp.
    pub async fn send_onboarding(&self, contact: &ContactId) -> Result<(), WhatsAppError> {
        let text = format!(
            "👋 *Welcome to Operon!*\n\n\
             I am your autonomous AI assistant running locally on Operon.\n\n\
             💡 *Shortcuts & Tips:*\n\
             • Send `/new` anytime to start a fresh, clean session.\n\
             • You can ask questions, run web searches, analyze files, and manage tasks.\n\n\
             _Starting your session now..._"
        );
        let msg = OutboundMessage::new(contact.as_str(), &text);
        let _ = self.outbound_tx.send(msg).await;
        Ok(())
    }

    /// Process a turn for a contact message over WhatsApp.
    ///
    /// This method mirrors `gui/src/executor/session.rs::start_agent_session` for session
    /// initialization: it loads prior history from the on-disk JSON SessionStore so that
    /// follow-up messages in the same session see the full conversation context.
    ///
    /// # Session lifecycle
    ///
    /// - **New session** (`is_first_time` AND no JSON file on disk): calls `store.create_session()`
    ///   to initialize the session record, then creates a cold `SessionRunner`.
    /// - **Existing session** (JSON file exists on disk): loads `history_turns`, computes
    ///   `turn_index` and `last_token_count`, then calls `runner.set_history()` after
    ///   `SessionRunner::new()` to restore conversation state.
    ///
    /// # Persistence
    ///
    /// Turn persistence is automatic inside `SessionRunner` — `loop_impl.rs` calls
    /// `store.save_turn()` after each assistant message. No explicit persistence call
    /// is needed here after the event loop completes.
    pub async fn process_turn(
        &self,
        contact: &ContactId,
        session_id: &str,
        role: CallerRole,
        user_message: String,
        is_first_time: bool,
    ) -> Result<(), WhatsAppError> {
        // Send onboarding doc on first message from this contact
        if is_first_time {
            let _ = self.send_onboarding(contact).await;
        }

        // 1. Provision user workspace & role-specific channel instructions
        let is_owner = matches!(role, CallerRole::Owner);
        let workspace_root = self
            .workspace_manager
            .provision_workspace(contact, is_owner)?;

        let channel_instructions = if is_owner {
            crate::workspace::generate_owner_channel_instructions(contact)
        } else {
            crate::workspace::generate_external_channel_instructions(contact)
        };

        // 2. Compute JSON session store path
        let store_path = self
            .workspace_manager
            .session_file_path_for(contact, session_id);

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
            compaction: operon_context::CompactionConfig::default(),
            store_path: Some(store_path.clone()),
            channel_instructions: Some(channel_instructions),
        };

        // ── 4. Session history loading ──────────────────────────────────────────
        // Mirrors gui/src/executor/session.rs::start_agent_session exactly.
        //
        // Determine is_new_session: true only when the router says is_first_time AND
        // the session JSON file does not already exist on disk. This handles the edge
        // case where a prior crashed/restarted turn already created the file — we
        // should still load its history rather than treating it as brand new.
        let is_new_session = is_first_time && !store_path.exists();

        // Open the SessionStore at store_path (creates parent dirs if needed).
        let store = SessionStore::open(&store_path)
            .await
            .map_err(|e| WhatsAppError::Session(e.to_string()))?;

        // For brand new sessions, create the session record in the JSON store first.
        // This matches gui/src/executor/session.rs's is_new_session branch exactly:
        // session_id, workspace_root as string, model_id, provider debug string.
        if is_new_session {
            store
                .create_session(
                    session_id,
                    &workspace_root.to_string_lossy(),
                    session_config.provider_config.model_id(),
                    &format!("{:?}", session_config.provider_config.provider),
                )
                .await
                .map_err(|e| WhatsAppError::Session(e.to_string()))?;
        }

        let history = store
            .load_full_history(session_id)
            .await
            .map_err(|e| WhatsAppError::Session(e.to_string()))?;

        // Load prior turn history from the store (empty vec if new session).
        let history_turns = store
            .load_turns(session_id)
            .await
            .map_err(|e| WhatsAppError::Session(e.to_string()))?;

        // turn_index is the count of previously completed turns — the runner uses this
        // to correctly number the next turn it executes.
        let turn_index = history_turns.len();

        // last_token_count lets the runner's token tracker resume from the correct
        // context window estimate instead of starting at zero.
        let last_token_count = store
            .get_last_token_count(session_id)
            .await
            .map_err(|e| WhatsAppError::Session(e.to_string()))?;

        info!(
            "WhatsApp session {} for contact {}: is_new={}, history_turns={}, turn_index={}, last_token_count={:?}",
            session_id, contact, is_new_session, history_turns.len(), turn_index, last_token_count
        );

        // ── 5. Create mpsc channels for SessionEvent and SessionCommand ─────
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(100);
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(10);

        // Wire cmd_tx to the router so /new can cancel in-flight turns!
        if let Some(ref router) = self.router {
            router.register_cmd_tx(contact, session_id, cmd_tx.clone()).await;
        }

        // ── 6. Instantiate SessionRunner and restore history ────────────────
        // SessionRunner::new() creates a fresh runner with empty messages and turn_index=0.
        // We then call set_history() to inject the loaded conversation state, exactly
        // mirroring gui/src/executor/session.rs::start_agent_session's pattern.
        let mut runner = SessionRunner::new(session_config, event_tx, cmd_rx)
            .await
            .map_err(|e| WhatsAppError::Session(e.to_string()))?;

        // If we have prior history, restore it on the runner so the model sees all
        // previous conversation context.
        if !history.is_empty() {
            runner.set_history(history, turn_index, last_token_count);
        }

        // ── 7. Spawn runner task ────────────────────────────────────────────
        let runner_handle =
            tokio::spawn(async move { runner.run(user_message, vec![], vec![]).await });

        // ── 8. Event consumer loop — forward tool progress & final text ─────
        forward_session_events_to_outbound(
            contact,
            session_id,
            &cmd_tx,
            self.event_hook.as_ref(),
            &self.outbound_tx,
            &mut event_rx,
        )
        .await;

        // Unregister cmd_tx from router upon turn completion
        if let Some(ref router) = self.router {
            router.unregister_cmd_tx(contact, session_id).await;
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
    contact: &ContactId,
    session_id: &str,
    cmd_tx: &mpsc::Sender<SessionCommand>,
    event_hook: Option<&SessionEventHook>,
    outbound_tx: &mpsc::Sender<OutboundMessage>,
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
                let out = OutboundMessage::new(contact.as_str(), &msg);
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::ToolCallStart { name, .. } => {
                let progress_msg = format!("⚡ *Executing:* `{}`", name);
                let out = OutboundMessage::new(contact.as_str(), &progress_msg);
                let _ = outbound_tx.send(out).await;
            }
            SessionEvent::TextDelta { text } => {
                accumulated_text.push_str(&text);
            }
            SessionEvent::Done => {
                send_final_text(contact, outbound_tx, &accumulated_text).await;
                terminal_event_seen = true;
                break;
            }
            SessionEvent::Error { message } => {
                error!("SessionRunner error for contact {}: {}", contact, message);
                let err_out =
                    OutboundMessage::new(contact.as_str(), &format!("❌ *Error:* {}", message));
                let _ = outbound_tx.send(err_out).await;
                terminal_event_seen = true;
                break;
            }
            other => {
                tracing::debug!(?other, "SessionEvent variant intentionally ignored for WhatsApp forwarding");
            }
        }
    }

    if !terminal_event_seen {
        send_final_text(contact, outbound_tx, &accumulated_text).await;
    }
}

async fn send_final_text(
    contact: &ContactId,
    outbound_tx: &mpsc::Sender<OutboundMessage>,
    accumulated_text: &str,
) {
    let trimmed = accumulated_text.trim();
    if !trimmed.is_empty() {
        let final_msg = OutboundMessage::new(contact.as_str(), trimmed);
        let _ = outbound_tx.send(final_msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn forwards_final_text_as_soon_as_done_arrives() {
        let contact = ContactId::new("15551112222");
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<OutboundMessage>(8);
        let held_sender = event_tx.clone();

        event_tx
            .send(SessionEvent::TextDelta {
                text: "Hello ".to_string(),
            })
            .await
            .unwrap();
        event_tx
            .send(SessionEvent::TextDelta {
                text: "from Operon".to_string(),
            })
            .await
            .unwrap();
        event_tx.send(SessionEvent::Done).await.unwrap();

        let (cmd_tx, _cmd_rx) = mpsc::channel::<SessionCommand>(1);
        tokio::time::timeout(
            Duration::from_secs(1),
            forward_session_events_to_outbound(&contact, "test-session", &cmd_tx, None, &outbound_tx, &mut event_rx),
        )
        .await
        .expect("Done must release the WhatsApp event forwarder");

        let out = outbound_rx.recv().await.unwrap();
        assert_eq!(out.recipient, "15551112222");
        assert_eq!(out.text, "Hello from Operon");
        assert!(outbound_rx.try_recv().is_err());
        drop(held_sender);
    }
}
