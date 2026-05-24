// Main entry point for Operon TUI
// Responsibilities:
// - Initialize terminal with crossterm (raw mode + alternate screen)
// - Set up panic hook to restore terminal before printing panic
// - Run main event loop: poll events → update state → render UI
// - Clean up terminal on exit (restore normal mode)

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;

mod agent;
mod error;
mod events;
mod state;
mod ui;

use std::sync::Arc;
use tokio::sync::Mutex;

use agent::{mock::MockAgent, AgentBridge};
use events::{action::Action, EventHandler};
use state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook that restores terminal before printing panic message
    // This prevents the terminal from being left in a broken state if we panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Initialize terminal with crossterm backend
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
    let result = run_app(&mut terminal, &mut app_state, &mut action_rx, agent, action_tx).await;

    // Restore terminal to normal state before exiting
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Print any error that occurred during execution
    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

/// Main application loop
/// Receives actions from event handler, updates state, and renders UI
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
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
            // Handle action and update state
            match action {
                Action::Quit => {
                    // Exit main loop and shut down application
                    break;
                }
                Action::Navigate(screen) => {
                    // Switch to a different screen
                    state.set_active_screen(screen);
                }
                Action::Back => {
                    // Go back to Chat screen
                    state.set_active_screen(state::screen::ActiveScreen::Chat);
                }
                Action::ToggleRightPanel(content) => {
                    // Open right panel with specified content
                    state.set_right_panel(Some(content));
                }
                Action::CloseRightPanel => {
                    // Hide right panel
                    state.set_right_panel(None);
                }
                Action::ToggleTerminal => {
                    // Toggle terminal panel (open if closed, close if open)
                    use ui::chrome::right_sidebar::panel_state::RightPanelContent;
                    if let Some(RightPanelContent::Terminal) = state.right_panel() {
                        state.set_right_panel(None);
                    } else {
                        state.set_right_panel(Some(RightPanelContent::Terminal));
                    }
                }
                Action::OpenFile(path) => {
                    // Open file preview in right panel
                    use ui::chrome::right_sidebar::panel_state::RightPanelContent;
                    state.set_right_panel(Some(RightPanelContent::FilePreview(path)));
                }
                Action::ProcessKey(key_event) => {
                    // Check if Ctrl+Shift is being held (for selection mode)
                    use crossterm::event::{KeyModifiers, KeyEventKind};
                    let ctrl_shift = key_event.modifiers.contains(KeyModifiers::CONTROL) 
                                  && key_event.modifiers.contains(KeyModifiers::SHIFT);
                    
                    // Detect when Ctrl+Shift is released - just clear the held state, don't copy
                    if key_event.kind == KeyEventKind::Release {
                        if state.is_ctrl_shift_held() && !ctrl_shift {
                            state.set_ctrl_shift_held(false);
                        }
                        continue; // Don't process release events as actions
                    }
                    
                    // Update Ctrl+Shift held state on press
                    if ctrl_shift != state.is_ctrl_shift_held() {
                        state.set_ctrl_shift_held(ctrl_shift);
                    }
                    
                    // Process key event with full state context (only for press events)
                    // Check if screen selector is open first
                    let action = if state.is_screen_selector_open() {
                        events::key::map_screen_selector_keys(key_event)
                    } else {
                        events::key::map_key(key_event, state.active_screen())
                    };

                    // Handle the mapped action
                    if let Some(action) = action {
                        match action {
                            Action::Quit => break,
                            Action::Navigate(screen) => state.set_active_screen(screen),
                            Action::Back => {
                                // If screen selector is open, close it
                                // Otherwise, go back to Chat screen
                                if state.is_screen_selector_open() {
                                    state.close_screen_selector();
                                } else {
                                    state.set_active_screen(state::screen::ActiveScreen::Chat);
                                }
                            }
                            Action::ToggleTerminal => {
                                use ui::chrome::right_sidebar::panel_state::RightPanelContent;
                                if let Some(RightPanelContent::Terminal) = state.right_panel() {
                                    state.set_right_panel(None);
                                } else {
                                    state.set_right_panel(Some(RightPanelContent::Terminal));
                                }
                            }
                            Action::CloseScreenSelector => {
                                state.close_screen_selector();
                            }
                            Action::ScreenSelectorUp => {
                                state.screen_selector_up();
                            }
                            Action::ScreenSelectorDown => {
                                state.screen_selector_down();
                            }
                            Action::ScreenSelectorConfirm => {
                                let selected = state.get_selected_screen();
                                state.set_active_screen(selected);
                                state.close_screen_selector();
                            }
                            Action::InputChar(c) => {
                                if c == '/' && state.is_input_empty() {
                                    state.open_screen_selector();
                                } else {
                                    // Let TextArea handle the character input
                                    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                    state.message_input_mut().input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                                }
                            }
                            Action::InputBackspace => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
                            }
                            Action::InputDelete => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
                            }
                            Action::InputNewline => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                            }
                            Action::InputCursorLeft => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
                            }
                            Action::InputCursorRight => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
                            }
                            Action::InputCursorUp => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
                            }
                            Action::InputCursorDown => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
                            }
                            Action::InputCursorHome => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
                            }
                            Action::InputCursorEnd => {
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                state.message_input_mut().input(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
                            }
                            Action::SendMessage => {
                                // Send message to agent
                                let message = state.get_input_text();
                                if !message.trim().is_empty() {
                                    // Add user message to history
                                    state.add_message("User".to_string(), message.clone());
                                    state.clear_input();
                                    
                                    // Mark agent as thinking — triggers spinner in status bar
                                    state.set_agent_thinking(true);
                                    
                                    // Send to agent asynchronously
                                    let agent_clone = Arc::clone(&agent);
                                    let action_tx_clone = action_tx.clone();
                                    tokio::spawn(async move {
                                        let agent_lock = agent_clone.lock().await;
                                        match agent_lock.send_message(&message).await {
                                            Ok(response) => {
                                                let _ = action_tx_clone.send(Action::AgentResponse(response)).await;
                                            }
                                            Err(e) => {
                                                let _ = action_tx_clone.send(Action::AgentResponse(
                                                    format!("Error: {}", e)
                                                )).await;
                                            }
                                        }
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Action::AgentResponse(response) => {
                    // Add agent response to message history and clear thinking state
                    state.set_agent_thinking(false);
                    state.add_message("Operon".to_string(), response);
                }
                Action::ProcessMouse(mouse_event) => {
                    use crossterm::event::MouseEventKind;
                    
                    let terminal_height = terminal.size()?.height;
                    let input_area_start = terminal_height.saturating_sub(6);
                    
                    // Check if Ctrl+Shift is held for selection mode
                    if state.is_ctrl_shift_held() {
                        // Selection mode: Ctrl+Shift + mouse drag
                        match mouse_event.kind {
                            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                                if mouse_event.row < input_area_start {
                                    state.start_selection(mouse_event.row, mouse_event.column);
                                }
                            }
                            MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                                if mouse_event.row < input_area_start {
                                    state.update_selection(mouse_event.row, mouse_event.column);
                                }
                            }
                            _ => {}
                        }
                    } else {
                        // Normal mode: mouse scrolling
                        match mouse_event.kind {
                            MouseEventKind::ScrollUp => {
                                if mouse_event.row >= input_area_start {
                                    state.scroll_input_up(1);
                                } else {
                                    state.scroll_chat_up(3);
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                if mouse_event.row >= input_area_start {
                                    state.scroll_input_down(1);
                                } else {
                                    state.scroll_chat_down(3);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Action::SetCtrlShiftHeld(held) => {
                    state.set_ctrl_shift_held(held);
                }
                Action::CopySelection => {
                    if let Some(text) = state.get_selected_text() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let _ = clipboard.set_text(text);
                        }
                    }
                    // Clear selection after copying
                    state.clear_selection();
                }
                Action::ScrollChatUp(amount) => {
                    state.scroll_chat_up(amount);
                }
                Action::ScrollChatDown(amount) => {
                    state.scroll_chat_down(amount);
                }
                Action::Tick => {
                    // Increment tick counter for animations (spinner, etc.)
                    state.tick();
                }
                Action::SendMessage => {
                    // TODO: Send message to agent via AgentBridge
                    // For now, just clear the input
                    state.clear_input();
                }
                _ => {
                    // Ignore other actions that are handled in ProcessKey
                }
            }
        }
    }

    Ok(())
}
