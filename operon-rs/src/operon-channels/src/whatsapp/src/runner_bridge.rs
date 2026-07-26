// runner_bridge.rs — Operon SessionRunner bridge for WhatsApp channel.
//
// Hey friend! This file bridges WhatsApp incoming messages to `operon_session::SessionRunner`.
//
// Flow per inbound turn:
//   1. Check if first-time user -> auto-send onboarding documentation over WhatsApp.
//   2. Provision contact workspace (`~/.operon/channels/whatsapp/workspace/<phone>/`) & `AGENTS.md`.
//   3. Compute JSON session store path (`~/.operon/sessions/whatsapp/<phone>/<session_id>.json`).
//   4. Construct `SessionConfig` with `Role::Owner` or `Role::External`.
//   5. Instantiate `SessionRunner` and execute turn.
//   6. Listen to `SessionEvent` stream:
//      - `ToolCallStart`: send tool progress update (e.g. `⚡ Running web_search...`).
//      - `TextDelta` / `Done`: send final formatted response payload back over WhatsApp socket.

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

use operon_config::AppConfig;
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::CallerRole;
use operon_session::{SessionConfig, SessionRunner};

use crate::error::WhatsAppError;
use crate::outbound::OutboundMessage;
use crate::router::WhatsAppRouter;
use crate::types::ContactId;
use crate::workspace::WhatsAppWorkspaceManager;

/// Bridge that drives `SessionRunner` for a specific contact and sends output over WhatsApp outbound channel.
pub struct SessionRunnerBridge {
    app_config: AppConfig,
    workspace_manager: WhatsAppWorkspaceManager,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    router: Option<Arc<WhatsAppRouter>>,
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
    pub async fn process_turn(
        &self,
        contact: &ContactId,
        session_id: &str,
        role: CallerRole,
        user_message: String,
        is_first_time: bool,
    ) -> Result<(), WhatsAppError> {
        // Send onboarding doc on first message
        if is_first_time {
            let _ = self.send_onboarding(contact).await;
        }

        // 1. Provision user workspace & role-specific AGENTS.md
        let is_owner = matches!(role, CallerRole::Owner);
        let workspace_root = self
            .workspace_manager
            .provision_workspace(contact, is_owner)?;

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
            workspace_root,
            role: context_role,
            tool_groups: vec!["fs".into(), "shell".into(), "web".into(), "todo".into()],
            compaction: operon_context::CompactionConfig::default(),
            store_path: Some(store_path),
        };

        // 4. Create mpsc channels for SessionEvent and SessionCommand
        let (event_tx, mut event_rx) = mpsc::channel::<SessionEvent>(100);
        let (cmd_tx, cmd_rx) = mpsc::channel::<SessionCommand>(10);

        // Wire cmd_tx to the router so /new can cancel in-flight turns!
        if let Some(ref router) = self.router {
            router.register_cmd_tx(contact, session_id, cmd_tx).await;
        }

        // 5. Instantiate SessionRunner
        let mut runner = SessionRunner::new(session_config, event_tx, cmd_rx)
            .await
            .map_err(|e| WhatsAppError::Session(e.to_string()))?;

        // 6. Spawn runner task
        let runner_handle = tokio::spawn(async move {
            runner.run(user_message).await
        });

        // 7. Event consumer loop — forward tool progress & final text
        let mut accumulated_text = String::new();

        while let Some(event) = event_rx.recv().await {
            match event {
                SessionEvent::ToolCallStart { name, .. } => {
                    let progress_msg = format!("⚡ *Executing:* `{}`", name);
                    let out = OutboundMessage::new(contact.as_str(), &progress_msg);
                    let _ = self.outbound_tx.send(out).await;
                }
                SessionEvent::TextDelta { text } => {
                    accumulated_text.push_str(&text);
                }
                SessionEvent::Error { message } => {
                    error!("SessionRunner error for contact {}: {}", contact, message);
                    let err_out = OutboundMessage::new(contact.as_str(), &format!("❌ *Error:* {}", message));
                    let _ = self.outbound_tx.send(err_out).await;
                }
                _ => {}
            }
        }

        // Unregister cmd_tx from router upon turn completion
        if let Some(ref router) = self.router {
            router.unregister_cmd_tx(contact, session_id).await;
        }

        // Wait for runner task to finish cleanly
        if let Err(e) = runner_handle.await {
            info!("Runner handle ended (may have been aborted/cancelled): {}", e);
        }

        // Send accumulated final text if non-empty
        let trimmed = accumulated_text.trim();
        if !trimmed.is_empty() {
            let final_msg = OutboundMessage::new(contact.as_str(), trimmed);
            let _ = self.outbound_tx.send(final_msg).await;
        }

        Ok(())
    }
}
