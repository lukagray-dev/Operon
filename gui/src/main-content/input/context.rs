//! Context Indicator controller.
//!
//! Handles formatting and displaying context window capacity usage and token statistics details.

use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Format token counts to strings like "15k/128k"
pub fn format_tokens(used: i32, total: i32) -> String {
    let format_k = |n: i32| -> String {
        if n >= 1000 {
            format!("{}k", n / 1000)
        } else {
            format!("{}", n)
        }
    };
    format!("{}/{}", format_k(used), format_k(total))
}

/// Register context indicator click callback.
pub fn wire_context(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
    window.on_context_clicked(move || {
        println!("[operon-gui][input] Context indicator clicked.");
    });
}
