//! Reasoning strength selector controller.
//!
//! Cycles through reasoning levels (Low, Medium, High) for prompt processing.

use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Register reasoning selector click callback to cycle reasoning levels.
pub fn wire_reasoning(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();

    window.on_reasoning_clicked(move || {
        if let Some(win) = window_weak.upgrade() {
            let current = win.get_selected_reasoning().to_string();
            let next = match current.as_str() {
                "Low" => "Medium",
                "Medium" => "High",
                _ => "Low",
            };
            println!(
                "[operon-gui][input] Reasoning strength clicked. Changing level to: {}",
                next
            );
            win.set_selected_reasoning(next.into());
        }
    });
}
