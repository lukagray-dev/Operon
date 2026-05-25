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
                                    // Special handling for models screen: go back to provider list if on setup
                                    use crate::ui::screens::models::state::ModelsStep;
                                    if matches!(state.active_screen(), state::screen::ActiveScreen::Models)
                                        && matches!(state.models.step, ModelsStep::Setup) {
                                        state.models.back_to_provider_list();
                                    } else {
                                        state.set_active_screen(state::screen::ActiveScreen::Chat);
                                    }
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
                                // Special case: '/' as first character opens screen selector
                                if c == '/' && state.is_input_empty() {
                                    state.open_screen_selector();
                                } else {
                                    // Forward to TextArea as a regular character
                                    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                    state.message_input_mut().input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
                                }
                            }
                            Action::ForwardKeyToInput(key_event) => {
                                // Pass the raw key event directly to tui-textarea.
                                // Handles arrows, Home, End, Backspace, Delete,
                                // Shift+Enter (newline), word-jump (Ctrl+Left/Right), etc.
                                // Undo/redo are NOT forwarded — they are handled above
                                // via InputUndo/InputRedo which call .undo()/.redo() directly.
                                state.message_input_mut().input(key_event);
                            }
                            Action::InputUndo => {
                                // Ctrl+Z — call tui-textarea's undo() directly.
                                // tui-textarea's native key for undo is Ctrl+U (Emacs-style),
                                // so we bypass key forwarding and call the method directly.
                                state.message_input_mut().undo();
                            }
                            Action::InputRedo => {
                                // Ctrl+Shift+Z — call tui-textarea's redo() directly.
                                // tui-textarea's native key for redo is Ctrl+R (Emacs-style),
                                // so we bypass key forwarding and call the method directly.
                                state.message_input_mut().redo();
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
                            // Models actions are handled in the outer match statement
                            // Re-send them through the channel so they get processed there
                            action @ (Action::ModelsUp 
                                    | Action::ModelsDown 
                                    | Action::ModelsLeft
                                    | Action::ModelsRight
                                    | Action::ModelsConfirm 
                                    | Action::ModelsNextField 
                                    | Action::ModelsFetchModels 
                                    | Action::ModelsToggleCompat 
                                    | Action::ModelsForwardKeyToInput(_)) => {
                                // Re-send to outer handler
                                let _ = action_tx.send(action).await;
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
                        match mouse_event.kind {
                            MouseEventKind::ScrollUp => {
                                // Route scroll to the correct panel based on active screen
                                match state.active_screen() {
                                    state::screen::ActiveScreen::Help => {
                                        // Help screen: scroll up towards top
                                        state.scroll_help_up(3);
                                    }
                                    _ => {
                                        // Chat screen: scroll input or chat history
                                        if mouse_event.row >= input_area_start {
                                            state.scroll_input_up(1);
                                        } else {
                                            state.scroll_chat_up(3);
                                        }
                                    }
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                match state.active_screen() {
                                    state::screen::ActiveScreen::Help => {
                                        // Help screen: scroll down towards bottom (capped at max)
                                        state.scroll_help_down(3, u16::MAX);
                                    }
                                    _ => {
                                        if mouse_event.row >= input_area_start {
                                            state.scroll_input_down(1);
                                        } else {
                                            state.scroll_chat_down(3);
                                        }
                                    }
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
                
                // ===== Models Screen Actions =====
                Action::ModelsUp => {
                    use crate::ui::screens::models::state::{ModelsStep, FetchStatus, Provider};
                    match state.models.step {
                        ModelsStep::ProviderList => {
                            // Navigate provider list
                            state.models.move_provider_up();
                        }
                        ModelsStep::Setup => {
                            // Check if we're in a text input field - if so, forward to TextArea
                            let is_in_text_field = match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    state.models.focused_field == 0 // API key field
                                }
                                Some(Provider::Custom) => {
                                    state.models.focused_field == 0 || state.models.focused_field == 2 // URL or API key
                                }
                                None => false,
                            };
                            
                            if is_in_text_field {
                                // Forward to TextArea (though Up/Down don't do much in single-line fields)
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                                match state.models.selected_provider {
                                    Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                        let _ = state.models.api_key_input.input(key_event);
                                    }
                                    Some(Provider::Custom) => {
                                        if state.models.focused_field == 0 {
                                            let _ = state.models.base_url_input.input(key_event);
                                        } else {
                                            let _ = state.models.api_key_input.input(key_event);
                                        }
                                    }
                                    None => {}
                                }
                            } else if matches!(state.models.fetch_status, FetchStatus::Success) {
                                // Navigate model list
                                state.models.move_model_up();
                            }
                        }
                    }
                }
                Action::ModelsDown => {
                    use crate::ui::screens::models::state::{ModelsStep, FetchStatus, Provider};
                    match state.models.step {
                        ModelsStep::ProviderList => {
                            // Navigate provider list
                            state.models.move_provider_down();
                        }
                        ModelsStep::Setup => {
                            // Check if we're in a text input field - if so, forward to TextArea
                            let is_in_text_field = match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    state.models.focused_field == 0 // API key field
                                }
                                Some(Provider::Custom) => {
                                    state.models.focused_field == 0 || state.models.focused_field == 2 // URL or API key
                                }
                                None => false,
                            };
                            
                            if is_in_text_field {
                                // Forward to TextArea (though Up/Down don't do much in single-line fields)
                                use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                                let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                                match state.models.selected_provider {
                                    Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                        let _ = state.models.api_key_input.input(key_event);
                                    }
                                    Some(Provider::Custom) => {
                                        if state.models.focused_field == 0 {
                                            let _ = state.models.base_url_input.input(key_event);
                                        } else {
                                            let _ = state.models.api_key_input.input(key_event);
                                        }
                                    }
                                    None => {}
                                }
                            } else if matches!(state.models.fetch_status, FetchStatus::Success) {
                                // Navigate model list
                                state.models.move_model_down();
                            }
                        }
                    }
                }
                Action::ModelsLeft => {
                    use crate::ui::screens::models::state::{ModelsStep, Provider};
                    if matches!(state.models.step, ModelsStep::Setup) {
                        // Check if we're on compat field - if so, toggle
                        if matches!(state.models.selected_provider, Some(Provider::Custom))
                            && state.models.focused_field == 1 {
                            state.models.toggle_compat_mode();
                        } else {
                            // Otherwise, forward to TextArea for cursor movement
                            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                            let key_event = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
                            match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    let _ = state.models.api_key_input.input(key_event);
                                }
                                Some(Provider::Custom) => {
                                    if state.models.focused_field == 0 {
                                        let _ = state.models.base_url_input.input(key_event);
                                    } else if state.models.focused_field == 2 {
                                        let _ = state.models.api_key_input.input(key_event);
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                }
                Action::ModelsRight => {
                    use crate::ui::screens::models::state::{ModelsStep, Provider};
                    if matches!(state.models.step, ModelsStep::Setup) {
                        // Check if we're on compat field - if so, toggle
                        if matches!(state.models.selected_provider, Some(Provider::Custom))
                            && state.models.focused_field == 1 {
                            state.models.toggle_compat_mode();
                        } else {
                            // Otherwise, forward to TextArea for cursor movement
                            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                            let key_event = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
                            match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    let _ = state.models.api_key_input.input(key_event);
                                }
                                Some(Provider::Custom) => {
                                    if state.models.focused_field == 0 {
                                        let _ = state.models.base_url_input.input(key_event);
                                    } else if state.models.focused_field == 2 {
                                        let _ = state.models.api_key_input.input(key_event);
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                }
                Action::ModelsConfirm => {
                    use crate::ui::screens::models::state::{ModelsStep, Provider, FetchStatus};
                    match state.models.step {
                        ModelsStep::ProviderList => {
                            // Confirm provider selection and move to setup
                            state.models.confirm_provider();
                        }
                        ModelsStep::Setup => {
                            // Check if we're on the API key field - if so, trigger fetch
                            let is_on_api_key_field = match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    // Only one field (API key), always field 0
                                    state.models.focused_field == 0
                                }
                                Some(Provider::Custom) => {
                                    // API key is field 2 (0=URL, 1=compat, 2=API key)
                                    state.models.focused_field == 2
                                }
                                None => false,
                            };
                            
                            if is_on_api_key_field && !matches!(state.models.fetch_status, FetchStatus::Fetching) {
                                // Trigger fetch
                                state.models.start_fetch();
                                
                                // Spawn async mock fetch task
                                let provider = state.models.selected_provider;
                                let action_tx_clone = action_tx.clone();
                                tokio::spawn(async move {
                                    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                                    
                                    let models = match provider {
                                        Some(Provider::Anthropic) => vec![
                                            "claude-opus-4-5".to_string(),
                                            "claude-sonnet-4-5".to_string(),
                                            "claude-haiku-4-5".to_string(),
                                        ],
                                        Some(Provider::OpenAI) => vec![
                                            "gpt-4o".to_string(),
                                            "gpt-4o-mini".to_string(),
                                            "gpt-4-turbo".to_string(),
                                            "o1".to_string(),
                                            "o1-mini".to_string(),
                                        ],
                                        Some(Provider::Custom) => vec![
                                            "model-1".to_string(),
                                            "model-2".to_string(),
                                            "model-3".to_string(),
                                        ],
                                        None => vec![],
                                    };
                                    
                                    let _ = action_tx_clone.send(Action::ModelsFetchComplete(models)).await;
                                });
                            } else if matches!(state.models.fetch_status, FetchStatus::Success) {
                                // If models are already fetched, Enter confirms the selected model
                                // TODO: Save configuration and return to Chat
                                state.set_active_screen(state::screen::ActiveScreen::Chat);
                            }
                        }
                    }
                }
                Action::ModelsNextField => {
                    use crate::ui::screens::models::state::ModelsStep;
                    if matches!(state.models.step, ModelsStep::Setup) {
                        state.models.next_field();
                    }
                }
                Action::ModelsFetchModels => {
                    use crate::ui::screens::models::state::{ModelsStep, FetchStatus, Provider};
                    // Only fetch if on setup screen and not already fetching
                    if matches!(state.models.step, ModelsStep::Setup) 
                        && !matches!(state.models.fetch_status, FetchStatus::Fetching) {
                        state.models.start_fetch();
                        
                        // Spawn async mock fetch task
                        let provider = state.models.selected_provider;
                        let action_tx_clone = action_tx.clone();
                        tokio::spawn(async move {
                            // Mock delay (800ms)
                            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                            
                            // Generate mock model list based on provider
                            let models = match provider {
                                Some(Provider::Anthropic) => vec![
                                    "claude-opus-4-5".to_string(),
                                    "claude-sonnet-4-5".to_string(),
                                    "claude-haiku-4-5".to_string(),
                                ],
                                Some(Provider::OpenAI) => vec![
                                    "gpt-4o".to_string(),
                                    "gpt-4o-mini".to_string(),
                                    "gpt-4-turbo".to_string(),
                                    "o1".to_string(),
                                    "o1-mini".to_string(),
                                ],
                                Some(Provider::Custom) => vec![
                                    "model-1".to_string(),
                                    "model-2".to_string(),
                                    "model-3".to_string(),
                                ],
                                None => vec![],
                            };
                            
                            // Send completion action
                            let _ = action_tx_clone.send(Action::ModelsFetchComplete(models)).await;
                        });
                    }
                }
                Action::ModelsFetchComplete(models) => {
                    // Complete the fetch operation with results
                    state.models.complete_fetch(models);
                }
                Action::ModelsToggleCompat => {
                    use crate::ui::screens::models::state::{ModelsStep, Provider};
                    // Only toggle if on Custom provider setup and compat field is focused
                    // Otherwise, Left/Right do nothing (they're not text input)
                    if matches!(state.models.step, ModelsStep::Setup)
                        && matches!(state.models.selected_provider, Some(Provider::Custom))
                        && state.models.focused_field == 1 {
                        state.models.toggle_compat_mode();
                    }
                }
                Action::ModelsForwardKeyToInput(key_event) => {
                    use crate::ui::screens::models::state::{ModelsStep, Provider};
                    if matches!(state.models.step, ModelsStep::Setup) {
                        // Forward key to the appropriate TextArea based on focused field
                        match state.models.selected_provider {
                            Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                // Only API key field (always focused)
                                let _ = state.models.api_key_input.input(key_event);
                            }
                            Some(Provider::Custom) => {
                                match state.models.focused_field {
                                    0 => { let _ = state.models.base_url_input.input(key_event); } // Base URL field
                                    1 => {} // Compat mode field (not text input)
                                    2 => { let _ = state.models.api_key_input.input(key_event); } // API key field
                                    _ => {}
                                }
                            }
                            None => {}
                        }
                    }
                }
                
                _ => {
                    // Ignore other actions that are handled in ProcessKey
                }
            }
        }
    }

    Ok(())
}
