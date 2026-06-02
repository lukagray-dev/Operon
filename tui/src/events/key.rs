// Key event mapping
// Maps crossterm KeyEvent to Action based on current screen context
// Same key can trigger different actions depending on which screen is active

use crate::events::action::Action;
use crate::state::screen::ActiveScreen;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
pub fn map_key(
    key: KeyEvent,
    active_screen: &ActiveScreen,
    state: &crate::state::AppState,
) -> Option<Action> {
    // Global keybinds that work on all screens
    match (key.code, key.modifiers) {
        // Quit application (Ctrl+Q only)
        (KeyCode::Char('q'), KeyModifiers::CONTROL) => return Some(Action::Quit),

        // Copy selected text (Ctrl+C)
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Some(Action::CopySelection),

        // Undo (Ctrl+Z) — tui-textarea uses Ctrl+U natively (Emacs), but we
        // intercept here and call .undo() directly for the standard convention.
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => return Some(Action::InputUndo),

        // Redo (Ctrl+Shift+Z) — tui-textarea uses Ctrl+R natively (Emacs), but we
        // intercept here and call .redo() directly for the standard convention.
        (KeyCode::Char('Z'), m) if m == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            return Some(Action::InputRedo);
        }

        // Toggle terminal panel
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            return Some(Action::ToggleTerminal);
        }

        // Toggle left sidebar (file explorer)
        (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
            return Some(Action::ToggleLeftSidebar);
        }

        _ => {}
    }

    // Screen-specific keybinds
    match active_screen {
        ActiveScreen::Chat => map_chat_keys(key),
        ActiveScreen::Models => map_models_keys(key),
        ActiveScreen::Permissions => map_permissions_keys(key, state),
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
/// Keys that need app-level handling are mapped to Actions.
/// All other keys (including Ctrl+Z undo, Ctrl+Y redo, word-jump, etc.)
/// are forwarded raw to tui-textarea via Action::ForwardKeyToInput.
fn map_chat_keys(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        // Send message (Ctrl+Enter)
        (KeyCode::Enter, KeyModifiers::CONTROL) => Some(Action::SendMessage),

        // Back / close screen selector (Esc)
        (KeyCode::Esc, _) => Some(Action::Back),

        // '/' as the very first character opens the screen selector.
        // We handle this in main.rs after checking is_input_empty(), so
        // we still need to route it through InputChar so that check runs.
        (KeyCode::Char('/'), KeyModifiers::NONE) => Some(Action::InputChar('/')),

        // Everything else — including Ctrl+Z (undo), Ctrl+Y (redo),
        // Shift+Enter (newline), arrows, Home, End, Backspace, Delete,
        // regular characters, word-jump (Ctrl+Left/Right), etc. —
        // is forwarded directly to tui-textarea which handles them natively.
        _ => Some(Action::ForwardKeyToInput(key)),
    }
}

/// Models screen keybinds
/// Provider list:
/// - Up/Down: Navigate providers
/// - Enter: Select provider and go to setup
/// - Esc: Back to Chat
/// Setup form:
/// - Up/Down: Navigate model list (when fetched) OR move cursor in text fields
/// - Left/Right: Move cursor in text fields OR toggle compat mode
/// - Tab: Next field
/// - All other keys: Forward to TextArea widget for text input
/// - Esc: Back to provider list
fn map_models_keys(key: KeyEvent) -> Option<Action> {
    use crate::events::action::Action;

    match (key.code, key.modifiers) {
        // Enter and Esc are always handled specially
        (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::ModelsConfirm),
        (KeyCode::Esc, KeyModifiers::NONE) => Some(Action::Back),

        // Tab for field navigation
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Action::ModelsNextField),

        // Up/Down - will be handled contextually (navigation vs text input)
        (KeyCode::Up, KeyModifiers::NONE) => Some(Action::ModelsUp),
        (KeyCode::Down, KeyModifiers::NONE) => Some(Action::ModelsDown),

        // Left/Right - will be handled contextually (compat toggle vs text input)
        (KeyCode::Left, KeyModifiers::NONE) => Some(Action::ModelsLeft),
        (KeyCode::Right, KeyModifiers::NONE) => Some(Action::ModelsRight),

        // Forward all other keys to TextArea for text input
        _ => Some(Action::ModelsForwardKeyToInput(key)),
    }
}

/// Permissions screen keybinds
/// - Tab: Switch section (Global↔Directory) or panel (DirList↔ToolTable)
/// - Up/Down: Navigate selection
/// - Enter: Expand/collapse group (or confirm in modal)
/// - Space: Open rule editor
/// - +: Add directory (Directory section only)
/// - -: Delete directory (Directory section, DirList focused only)
/// - Esc: Back to Chat screen (or close modal if open)
fn map_permissions_keys(key: KeyEvent, state: &crate::state::AppState) -> Option<Action> {
    // Check if any modal is open
    let modal_open = state.permissions.rule_editor.open || state.permissions.add_dir.open;

    if modal_open {
        // Modal-specific keybinds
        if state.permissions.rule_editor.open {
            // Rule editor modal keybinds
            match (key.code, key.modifiers) {
                (KeyCode::Esc, KeyModifiers::NONE) => Some(Action::PermCloseModal),
                (KeyCode::Up, KeyModifiers::NONE) => Some(Action::PermEditorUp),
                (KeyCode::Down, KeyModifiers::NONE) => Some(Action::PermEditorDown),
                (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::PermEditorConfirm),
                (KeyCode::Tab, KeyModifiers::NONE) => Some(Action::PermEditorSwitchRole),
                _ => None,
            }
        } else if state.permissions.add_dir.open {
            // Add directory modal keybinds
            match (key.code, key.modifiers) {
                (KeyCode::Esc, KeyModifiers::NONE) => Some(Action::PermCloseModal),
                (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::PermEditorConfirm),
                // Forward all other keys to TextArea
                _ => Some(Action::PermForwardKeyToInput(key)),
            }
        } else {
            None
        }
    } else {
        // Normal screen keybinds
        match (key.code, key.modifiers) {
            (KeyCode::Esc, KeyModifiers::NONE) => Some(Action::Back),
            (KeyCode::Tab, KeyModifiers::NONE) => Some(Action::PermSwitchSection),
            (KeyCode::Up, KeyModifiers::NONE) => Some(Action::PermSelectUp),
            (KeyCode::Down, KeyModifiers::NONE) => Some(Action::PermSelectDown),
            (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::PermToggleExpand),
            (KeyCode::Char(' '), KeyModifiers::NONE) => Some(Action::PermOpenEditor),
            (KeyCode::Char('+'), KeyModifiers::NONE) => Some(Action::PermAddDirectory),
            (KeyCode::Char('-'), KeyModifiers::NONE) => Some(Action::PermDeleteDirectory),
            _ => None,
        }
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
