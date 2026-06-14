// commands.rs — Command channel handling helpers.
//
// Hey friend! This file manages reading and processing incoming command messages
// from the channel. We drain commands from the tokio receiver, buffer them in a
// queue (so we don't lose them if they are not immediately relevant), and search
// for target commands (like Approve/Deny/AskResponse for a specific ID).

use std::collections::VecDeque;
use tokio::sync::mpsc;
use operon_events::SessionCommand;

/// Return true if a buffered command should be consumed for the current state.
pub fn command_matches(command: &SessionCommand, approval_id: Option<&str>) -> bool {
    // Hey friend! Here we check if the command matches the expected command type.
    // A Cancel command always matches. Approve, Deny, and AskResponse commands
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

/// Move any immediately available inbound commands into the local buffer.
///
/// This prevents us from dropping commands when we only want to inspect
/// whether a Cancel is pending before dispatching the next tool call.
pub fn drain(
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
    pending_commands: &mut VecDeque<SessionCommand>,
) {
    while let Ok(command) = cmd_rx.try_recv() {
        pending_commands.push_back(command);
    }
}

/// Remove the first buffered command that matches the requested approval.
///
/// `approval_id = None` means only `Cancel` is relevant. When an approval ID
/// is present, `Approve` and `Deny` must match that ID.
pub fn take_matching(
    pending_commands: &mut VecDeque<SessionCommand>,
    approval_id: Option<&str>,
) -> Option<SessionCommand> {
    let index = pending_commands
        .iter()
        .position(|command| command_matches(command, approval_id))?;
    pending_commands.remove(index)
}

/// Wait until the command channel yields something relevant to the current
/// approval request or a cancel signal.
///
/// Irrelevant commands are buffered so we do not lose them.
pub async fn wait_for_relevant(
    cmd_rx: &mut mpsc::Receiver<SessionCommand>,
    pending_commands: &mut VecDeque<SessionCommand>,
    approval_id: Option<&str>,
) -> SessionCommand {
    loop {
        if let Some(command) = take_matching(pending_commands, approval_id) {
            return command;
        }

        match cmd_rx.recv().await {
            Some(command) => pending_commands.push_back(command),
            None => return SessionCommand::Cancel,
        }
    }
}
