// mouse.rs — Mouse interactions and clipboard copy handlers for Operon TUI.
//
// ZERO BUSINESS LOGIC IN FRONTEND:
// Pure UI state updates for mouse dragging, text selection, and scrolling.

use crate::events::action::Action;
use crate::state::AppState;
use anyhow::Result;

/// Handle mouse events, selection mode, and clipboard copy operations.
pub async fn handle(action: Action, state: &mut AppState, terminal_height: u16) -> Result<()> {
    match action {
        Action::ProcessMouse(mouse_event) => {
            use crossterm::event::MouseEventKind;

            let input_area_start = terminal_height.saturating_sub(6);

            // Check if Ctrl+Shift is held for selection mode
            if state.is_ctrl_shift_held() {
                // Selection mode: Ctrl+Shift + mouse drag
                match mouse_event.kind {
                    MouseEventKind::Down(crossterm::event::MouseButton::Left)
                        if mouse_event.row < input_area_start =>
                    {
                        state.start_selection(mouse_event.row, mouse_event.column);
                    }
                    MouseEventKind::Drag(crossterm::event::MouseButton::Left)
                        if mouse_event.row < input_area_start =>
                    {
                        state.update_selection(mouse_event.row, mouse_event.column);
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
        _ => {}
    }

    Ok(())
}
