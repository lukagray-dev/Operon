//! Main Content area controller.
//!
//! This module orchestrates the wiring of main content callbacks (prompt input, message actions)
//! and delegates sub-component wiring to child modules.

#[path = "input/input.rs"]
pub mod input;

use std::cell::RefCell;
use std::rc::Rc;
use crate::state::AppState;

/// Wire all callbacks and update properties inside the main content view.
pub fn wire_main_content(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    // Wire prompt input area
    input::wire_input_panel(window, Rc::clone(&state));
}
