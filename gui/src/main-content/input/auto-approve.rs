//! Auto-Approve toggle button controller.
//!
//! This module manages the auto-approve policy configuration state for prompt execution.

use slint::ComponentHandle;
use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Register auto-approve toggle callback.
pub fn wire_auto_approve(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    let window_weak = window.as_weak();

    window.on_auto_approve_toggled(move |enabled| {
        println!("[operon-gui][input] Auto-approve toggled: {}", enabled);

        if let Some(win) = window_weak.upgrade() {
            win.set_auto_approve_enabled(enabled);
        }
    });
}
