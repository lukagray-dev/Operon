// Mouse and keyboard event handlers
// Handles: ProcessMouse, SetCtrlShiftHeld, CopySelection, ProcessKey
// These actions manage mouse interactions, selection mode, and raw key event processing

use crate::events::action::Action;
use crate::state::AppState;
use anyhow::Result;
use tokio::sync::mpsc;

/// Handle mouse and keyboard event actions
/// Processes mouse events (scrolling, selection), Ctrl+Shift state, clipboard operations,
/// and raw key events with full state context (including nested action dispatch)
pub async fn handle(
    action: Action,
    state: &mut AppState,
    tx: &mpsc::Sender<Action>,
    terminal_height: u16,
) -> Result<()> {
    match action {
        Action::ProcessMouse(mouse_event) => {
            use crossterm::event::MouseEventKind;

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
                            crate::state::screen::ActiveScreen::Help => {
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
                            crate::state::screen::ActiveScreen::Help => {
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
            // Update Ctrl+Shift held state (for selection mode)
            state.set_ctrl_shift_held(held);
        }
        Action::CopySelection => {
            // Copy selected text to clipboard
            if let Some(text) = state.get_selected_text() {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(text);
                }
            }
            // Clear selection after copying
            state.clear_selection();
        }
        Action::ProcessKey(key_event) => {
            // Check if Ctrl+Shift is being held (for selection mode)
            use crossterm::event::{KeyEventKind, KeyModifiers};
            let ctrl_shift = key_event.modifiers.contains(KeyModifiers::CONTROL)
                && key_event.modifiers.contains(KeyModifiers::SHIFT);

            // Detect when Ctrl+Shift is released - just clear the held state, don't copy
            if key_event.kind == KeyEventKind::Release {
                if state.is_ctrl_shift_held() && !ctrl_shift {
                    state.set_ctrl_shift_held(false);
                }
                return Ok(()); // Don't process release events as actions
            }

            // Update Ctrl+Shift held state on press
            if ctrl_shift != state.is_ctrl_shift_held() {
                state.set_ctrl_shift_held(ctrl_shift);
            }

            // Process key event with full state context (only for press events)
            // Check if screen selector is open first
            let mapped_action = if state.is_screen_selector_open() {
                crate::events::key::map_screen_selector_keys(key_event)
            } else {
                crate::events::key::map_key(key_event, state.active_screen(), state)
            };

            // Handle the mapped action
            // This is a nested match that re-dispatches some actions back through the channel
            if let Some(inner_action) = mapped_action {
                match inner_action {
                    Action::Quit => {
                        // Re-send Quit to trigger loop break in app::run
                        let _ = tx.send(Action::Quit).await;
                    }
                    Action::Navigate(screen) => {
                        state.set_active_screen(screen);
                    }
                    Action::Back => {
                        // If screen selector is open, close it
                        // Otherwise, go back to Chat screen
                        if state.is_screen_selector_open() {
                            state.close_screen_selector();
                        } else {
                            // Special handling for models screen: go back to provider list if on setup
                            use crate::ui::screens::models::state::ModelsStep;
                            if matches!(
                                state.active_screen(),
                                crate::state::screen::ActiveScreen::Models
                            ) && matches!(state.models.step, ModelsStep::Setup)
                            {
                                state.models.back_to_provider_list();
                            } else {
                                state.set_active_screen(crate::state::screen::ActiveScreen::Chat);
                            }
                        }
                    }
                    Action::ToggleTerminal => {
                        use crate::ui::chrome::right_sidebar::panel_state::RightPanelContent;
                        if let Some(RightPanelContent::Terminal) = state.right_panel() {
                            state.set_right_panel(None);
                        } else {
                            state.set_right_panel(Some(RightPanelContent::Terminal));
                        }
                    }
                    Action::ToggleLeftSidebar => {
                        state.toggle_left_sidebar();
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
                            state
                                .message_input_mut()
                                .input(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
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
                        // Re-send through channel to be handled by input handler
                        let _ = tx.send(Action::SendMessage).await;
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
                        let _ = tx.send(action).await;
                    }
                    // Permissions actions are handled in the outer match statement
                    // Re-send them through the channel so they get processed there
                    action @ (Action::PermSwitchSection
                    | Action::PermSelectUp
                    | Action::PermSelectDown
                    | Action::PermToggleExpand
                    | Action::PermOpenEditor
                    | Action::PermAddDirectory
                    | Action::PermDeleteDirectory
                    | Action::PermCloseModal
                    | Action::PermEditorUp
                    | Action::PermEditorDown
                    | Action::PermEditorConfirm
                    | Action::PermEditorSwitchRole
                    | Action::PermForwardKeyToInput(_)) => {
                        // Re-send to outer handler
                        let _ = tx.send(action).await;
                    }
                    _ => {}
                }
            }
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }

    Ok(())
}
