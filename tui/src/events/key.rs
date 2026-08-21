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

        // Paste from clipboard (Ctrl+V, Ctrl+Shift+V, Shift+Insert)
        (KeyCode::Char('v'), KeyModifiers::CONTROL) => return Some(Action::PasteClipboard),
        (KeyCode::Char('V'), m) if m == (KeyModifiers::CONTROL | KeyModifiers::SHIFT) => {
            return Some(Action::PasteClipboard);
        }
        (KeyCode::Insert, KeyModifiers::SHIFT) => return Some(Action::PasteClipboard),

        _ => {}
    }

    // Screen-specific keybinds
    match active_screen {
        ActiveScreen::Chat => map_chat_keys(key, state),
        ActiveScreen::Resume => map_resume_keys(key),
        ActiveScreen::Models => map_models_keys(key),
        ActiveScreen::Permissions => map_permissions_keys(key, state),
        ActiveScreen::Skills => map_skills_keys(key),
        ActiveScreen::Extensions => map_extensions_keys(key),
        ActiveScreen::Help => map_help_keys(key),
    }
}

/// Resume screen keybinds
/// - Up/Down (or j/k): Navigate previous conversations
/// - Enter: Select and load conversation
/// - Esc: Return to chat
/// - /: Open screen selector
fn map_resume_keys(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
            Some(Action::ResumeUp)
        }
        (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
            Some(Action::ResumeDown)
        }
        (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::ResumeConfirm),
        (KeyCode::Esc, _) => Some(Action::Back),
        (KeyCode::Char('/'), KeyModifiers::NONE) => Some(Action::InputChar('/')),
        _ => None,
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

/// Chat screen keybinds:
/// - Esc (when thinking): Cancel prompt generation
/// - Enter / Ctrl+Enter: Send message
/// - Shift+Enter: Insert newline
/// - /: Screen selector (when input empty)
/// - All other keys forwarded to tui-textarea
fn map_chat_keys(key: KeyEvent, state: &crate::state::AppState) -> Option<Action> {
    // If agent is actively running/thinking, Esc or Ctrl+C triggers cancellation
    if state.agent_thinking()
        && (key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL))
    {
        return Some(Action::CancelPrompt);
    }

    match (key.code, key.modifiers) {
        // Send message (Ctrl+Enter or Enter)
        (KeyCode::Enter, KeyModifiers::CONTROL) | (KeyCode::Enter, KeyModifiers::NONE) => {
            Some(Action::SendMessage)
        }

        // Shift+Enter inserts newline into input textarea
        (KeyCode::Enter, KeyModifiers::SHIFT) => Some(Action::ForwardKeyToInput(key)),

        // Back / close screen selector (Esc)
        (KeyCode::Esc, _) => Some(Action::Back),

        // '/' as the very first character opens the screen selector.
        (KeyCode::Char('/'), KeyModifiers::NONE) => Some(Action::InputChar('/')),

        // Everything else forwarded directly to tui-textarea
        _ => Some(Action::ForwardKeyToInput(key)),
    }
}

/// Models screen keybinds
///
/// Provider list:
/// - Up/Down: Navigate providers
/// - Enter: Select provider and go to setup
/// - Esc: Back to Chat
///
/// Setup form:
/// - Up/Down: Navigate model list (when fetched) OR move cursor in text fields
/// - Left/Right: Move cursor in text fields OR toggle compat mode
/// - Tab: Next field
/// - All other keys: Forward to TextArea widget for text input
/// - Esc: Back to provider list
fn map_models_keys(key: KeyEvent) -> Option<Action> {
    use crate::events::action::Action;

    match (key.code, key.modifiers) {
        // Confirm / Select (Enter)
        (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::ModelsConfirm),

        // Back to previous screen / provider list (Esc)
        (KeyCode::Esc, KeyModifiers::NONE) => Some(Action::Back),

        // Field navigation: Tab = next, BackTab / Shift+Tab = previous
        (KeyCode::Tab, KeyModifiers::NONE) => Some(Action::ModelsNextField),
        (KeyCode::BackTab, _) | (KeyCode::Tab, KeyModifiers::SHIFT) => {
            Some(Action::ModelsPrevField)
        }

        // Model discovery trigger (Ctrl+F)
        (KeyCode::Char('f'), KeyModifiers::CONTROL) => Some(Action::ModelsFetchModels),

        // Save and activate provider (Ctrl+S)
        (KeyCode::Char('s'), KeyModifiers::CONTROL) => Some(Action::ModelsSaveProvider),

        // Toggle API key visibility (F2)
        (KeyCode::F(2), _) => Some(Action::ModelsToggleKeyVisibility),

        // Up/Down navigation (contextual in handler)
        (KeyCode::Up, KeyModifiers::NONE) => Some(Action::ModelsUp),
        (KeyCode::Down, KeyModifiers::NONE) => Some(Action::ModelsDown),

        // Left/Right cursor movement
        (KeyCode::Left, KeyModifiers::NONE) => Some(Action::ModelsLeft),
        (KeyCode::Right, KeyModifiers::NONE) => Some(Action::ModelsRight),

        // Forward all other character/editing keystrokes to the focused TextArea
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

/// Help screen keybinds
/// - Esc: Back to Chat screen
fn map_help_keys(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::Back),
        _ => None,
    }
}
