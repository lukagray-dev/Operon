// Main entry point for Operon TUI
// Responsibilities:
// - Install panic hook to restore terminal on panic
// - Initialize terminal with crossterm (raw mode + alternate screen + mouse capture)
// - Set up event handler channel
// - Run main event loop via app::run
// - Clean up terminal on exit (restore normal mode)

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

mod agent;
mod app;
mod error;
mod events;
mod state;
mod ui;

use agent::{mock::MockAgent, AgentBridge};
use events::{action::Action, EventHandler};
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook that restores terminal before printing panic message
    app::terminal::install_panic_hook();

    // Initialize terminal with crossterm backend
    let mut terminal = app::terminal::init()?;

    // Initialize application state with mock agent
    let agent: Arc<Mutex<Box<dyn AgentBridge>>> = Arc::new(Mutex::new(Box::new(MockAgent::new())));
    let mut app_state = AppState::new();

    // Create event handler channel
    // EventHandler runs in a separate thread and sends Action events to main loop
    let (action_tx, mut action_rx) = mpsc::channel::<Action>(100);
    let event_handler = EventHandler::new(action_tx.clone());
    event_handler.start();

    // Main event loop
    // Poll for actions → update state → render UI → repeat until quit
    let result = app::run(
        &mut terminal,
        &mut app_state,
        &mut action_rx,
        agent,
        action_tx,
    )
    .await;

    // Restore terminal to normal state before exiting
    app::terminal::restore(&mut terminal)?;

    // Print any error that occurred during execution
    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}
