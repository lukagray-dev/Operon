// runner/commands.rs — Command-channel plumbing for SessionRunner.
//
// This module contains the inbound command handling machinery:
// draining buffered commands, matching commands against pending approval
// requests, and waiting asynchronously for relevant commands to arrive.
// Also hosts the `caller_role` and `tool_progress_emitter` helpers.

use std::sync::Arc;

use operon_context::Role;
use operon_events::{SessionCommand, SessionEvent};
use operon_policy::CallerRole;
use operon_tools::ToolProgressEmitter;

use super::message_build::context_usage_event;
use super::SessionRunner;

impl SessionRunner {
    /// Convert the session runtime role into the policy crate role.
    pub(super) fn caller_role(&self) -> CallerRole {
        match self.config.role {
            Role::Owner => CallerRole::Owner,
            Role::External => CallerRole::External,
        }
    }

    /// Move any immediately available inbound commands into the local buffer.
    ///
    /// This prevents us from dropping commands when we only want to inspect
    /// whether a Cancel is pending before dispatching the next tool call.
    pub(super) fn drain_ready_commands(&mut self) {
        while let Ok(command) = self.cmd_rx.try_recv() {
            self.pending_commands.push_back(command);
        }
    }

    /// Emit the current context-window gauge for the UI.
    pub(super) async fn emit_context_usage_update(&self) {
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
    pub(super) fn tool_progress_emitter(&self) -> ToolProgressEmitter {
        let event_tx = self.event_tx.clone();

        Arc::new(move |progress| {
            let _ = event_tx.try_send(SessionEvent::ToolProgress(progress));
        })
    }

    /// Remove the first buffered command that matches the requested approval.
    ///
    /// `approval_id = None` means only `Cancel` is relevant. When an approval ID
    /// is present, `Approve` and `Deny` must match that ID.
    pub(super) fn take_matching_command(
        &mut self,
        approval_id: Option<&str>,
    ) -> Option<SessionCommand> {
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
    pub(super) async fn wait_for_relevant_command(
        &mut self,
        approval_id: Option<&str>,
    ) -> SessionCommand {
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
pub(super) fn command_matches(command: &SessionCommand, approval_id: Option<&str>) -> bool {
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
