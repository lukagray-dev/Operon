// Application module
// Contains the main event loop and terminal management
// Responsibilities:
// - Run the main event loop: draw → recv → dispatch → repeat
// - Coordinate between terminal rendering and action handling

use anyhow::Result;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::Stdout;
use std::ops::ControlFlow;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::agent::AgentBridge;
use crate::events::action::Action;
use crate::state::AppState;
use crate::ui;

// Submodules
mod handlers;
pub mod terminal;

/// Main application event loop
/// Receives actions from event handler, dispatches them to handlers, and renders UI
///
/// Loop structure:
/// 1. Draw current state to terminal
/// 2. Wait for next action from channel
/// 3. Dispatch action to appropriate handler
/// 4. Check if we should break (Quit action)
/// 5. Repeat
pub async fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    action_rx: &mut mpsc::Receiver<Action>,
    agent: Arc<Mutex<Box<dyn AgentBridge>>>,
    action_tx: mpsc::Sender<Action>,
) -> Result<()> {
    loop {
        // Render current state to terminal
        terminal.draw(|frame| ui::render(frame, state))?;

        // Wait for next action from event handler
        if let Some(action) = action_rx.recv().await {
            // Get terminal height for mouse handler (needed for input area calculation)
            let terminal_height = terminal.size()?.height;

            // Dispatch action to appropriate handler
            let flow =
                handlers::dispatch(action, state, &agent, &action_tx, terminal_height).await?;

            // Check if we should exit the loop
            if flow == ControlFlow::Break(()) {
                break;
            }
        }
    }

    Ok(())
}
