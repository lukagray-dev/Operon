//! Appearance Settings panel controller.
//!
//! This module intentionally wires only settings that have backend persistence
//! today. The rest of the Appearance page remains visual-only until those
//! preferences are modeled in `GuiPrefs`.

use crate::settings::prefs::ThinkingOrbStyle;
use crate::state::AppState;
use std::cell::RefCell;
use std::rc::Rc;

/// Initializes and binds persisted interaction callbacks for the Appearance settings panel.
pub fn wire_appearance_settings(window: &crate::SettingsWindow, state: Rc<RefCell<AppState>>) {
    {
        let app_state = state.borrow();
        window
            .set_appearance_selected_thinking_orb(app_state.prefs().thinking_orb_style.to_index());
    }

    let state_orb = Rc::clone(&state);
    window.on_appearance_thinking_orb_changed(move |idx| {
        eprintln!("[operon-gui][appearance] Thinking orb changed: {idx}");
        state_orb
            .borrow_mut()
            .set_thinking_orb_style(ThinkingOrbStyle::from_index(idx));
    });
}
