// Event handling module
// Spawns a background thread that polls crossterm events
// Converts raw key events to Actions and sends them to main loop via channel
// Also sends periodic Tick actions for animations

pub mod action;
pub mod key;

use crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;
use tokio::sync::mpsc;
use crate::events::action::Action;

/// EventHandler manages the event polling thread
/// Polls crossterm for keyboard/mouse events and converts them to Actions
/// Also sends periodic Tick actions for animations
pub struct EventHandler {
    /// Channel sender for sending Actions to main loop
    action_tx: mpsc::Sender<Action>,
}

impl EventHandler {
    /// Create a new EventHandler with the given action channel sender
    pub fn new(action_tx: mpsc::Sender<Action>) -> Self {
        Self { action_tx }
    }

    /// Start the event polling thread
    /// Spawns a tokio task that:
    /// 1. Polls crossterm for events with a timeout
    /// 2. Sends raw key events to main loop for processing with full state context
    /// 3. Sends Tick actions every 100ms for animations
    pub fn start(self) {
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    if let Ok(event) = event::read() {
                        match event {
                            Event::Key(key_event) => {
                                // Only process key press events, ignore key release
                                // This prevents duplicate actions from press+release
                                if key_event.kind != KeyEventKind::Press {
                                    continue;
                                }

                                // Send raw key event to main loop for context-aware processing
                                if self.action_tx.send(Action::ProcessKey(key_event)).await.is_err() {
                                    break;
                                }
                            }
                            Event::Mouse(mouse_event) => {
                                // Send mouse events to main loop
                                if self.action_tx.send(Action::ProcessMouse(mouse_event)).await.is_err() {
                                    break;
                                }
                            }
                            // TODO: Handle terminal resize events
                            Event::Resize(_, _) => {}
                            _ => {}
                        }
                    }
                } else {
                    // Timeout reached, send Tick action for animations
                    if self.action_tx.send(Action::Tick).await.is_err() {
                        break;
                    }
                }
            }
        });
    }
}
