//! Text input controller.
//!
//! Exposes setup handlers for prompt text input fields.

use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Register prompt text callbacks.
pub fn wire_text(
    _window: &crate::OperonWindow,
    _state: Rc<RefCell<AppState>>,
) {
    // Currently text updates are synchronized bi-directionally via Slint properties.
    // Keystroke listeners are not registered by default.
}
