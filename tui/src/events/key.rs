// Key event mapping
// Maps crossterm KeyEvent to Action based on current screen context
// Same key can trigger different actions depending on which screen is active

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::events::action::Action;
use crate::state::screen::ActiveScreen;

/// Map a key event to an Action based on current screen context
/// Returns None if the key has no mapped action in the current context
/// 
/// Global keybinds (work on all screens):
/// - Ctrl+Q: Quit application
/// - Ctrl+C: Quit application
/// - Ctrl+T: Toggle terminal panel
/// - Esc: Back to previous screen (eventually Chat)
/// - /: Open screen selector (when in input)
/// 
/// Screen selector keybinds (when selector is open):
/// - Up/Down: Navigate
/// - Enter: Confirm selection
/// - Esc: Close selector
pub fn map_key(key: KeyEvent, active_screen: &ActiveScreen) -> Option<Action> {
    // Global keybinds that work on all screens
    match (key.code, key.modifiers) {
        // Quit application
        (KeyCode::Char('q'), KeyModifiers::CONTROL) => return Some(Action::Quit),
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(Action::Quit),
        
        // Toggle terminal panel
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            return Some(Action::ToggleTerminal);
        }
        
        _ => {}
    }

    // Screen-specific keybinds
    match active_screen {
        ActiveScreen::Chat => map_chat_keys(key),
        ActiveScreen::Models => map_models_keys(key),
        ActiveScreen::Permissions => map_permissions_keys(key),
        ActiveScreen::Skills => map_skills_keys(key),
        ActiveScreen::Extensions => map_extensions_keys(key),
        ActiveScreen::Channels => map_channels_keys(key),
        ActiveScreen::Help => map_help_keys(key),
    }
}

/// Map keys when screen selector is open
/// This is called from the main loop when selector state is active
pub fn map_screen_selector_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Up => Some(Action::ScreenSelectorUp),
        KeyCode::Down => Some(Action::ScreenSelectorDown),
        KeyCode::Enter => Some(Action::ScreenSelectorConfirm),
        KeyCode::Esc => Some(Action::CloseScreenSelector),
        _ => None,
    }
}

/// Chat screen keybinds
/// - Ctrl+Enter: Send message
/// - Shift+Enter: Insert newline
/// - /: Open screen selector (when first character)
/// - Backspace: Delete character before cursor
/// - Delete: Delete character at cursor
/// - Left/Right arrows: Move cursor
/// - Up/Down arrows: Move cursor between lines
/// - Home: Move cursor to start
/// - End: Move cursor to end
/// - Esc: Back to previous screen
/// - Any printable character: Insert into input at cursor position
fn map_chat_keys(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        // Send message
        (KeyCode::Enter, KeyModifiers::CONTROL) => Some(Action::SendMessage),
        
        // Insert newline
        (KeyCode::Enter, KeyModifiers::SHIFT) => Some(Action::InputNewline),
        
        // Backspace
        (KeyCode::Backspace, _) => Some(Action::InputBackspace),
        
        // Delete
        (KeyCode::Delete, _) => Some(Action::InputDelete),
        
        // Cursor movement
        (KeyCode::Left, _) => Some(Action::InputCursorLeft),
        (KeyCode::Right, _) => Some(Action::InputCursorRight),
        (KeyCode::Up, _) => Some(Action::InputCursorUp),
        (KeyCode::Down, _) => Some(Action::InputCursorDown),
        (KeyCode::Home, _) => Some(Action::InputCursorHome),
        (KeyCode::End, _) => Some(Action::InputCursorEnd),
        
        // Back button
        (KeyCode::Esc, _) => Some(Action::Back),
        
        // Regular character input (no modifiers or only SHIFT for uppercase)
        (KeyCode::Char(c), KeyModifiers::NONE) => {
            Some(Action::InputChar(c))
        }
        (KeyCode::Char(c), KeyModifiers::SHIFT) => {
            Some(Action::InputChar(c))
        }
        
        _ => None,
    }
}

/// Models screen keybinds
/// - Esc: Back to Chat screen
fn map_models_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}

/// Permissions screen keybinds
/// - Esc: Back to Chat screen
fn map_permissions_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}

/// Skills screen keybinds
/// - Esc: Back to Chat screen
fn map_skills_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}

/// Extensions screen keybinds
/// - Esc: Back to Chat screen
fn map_extensions_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}

/// Channels screen keybinds
/// - Esc: Back to Chat screen
fn map_channels_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}

/// Help screen keybinds
/// - Esc: Back to Chat screen
fn map_help_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}
