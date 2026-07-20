//! Main Input Panel Coordinator.
//!
//! This module delegates sub-component wiring (attachment, models selection,
//! reasoning, auto-approve policy, send logic, text changes) to sibling files.

pub mod attach;
#[path = "auto-approve.rs"]
pub mod auto_approve;
pub mod context;
pub mod models;
pub mod reasoning;
pub mod send;
pub mod text;
pub mod voice;

use std::cell::RefCell;
use std::rc::Rc;

use crate::state::AppState;

/// Initialize and wire all input panel components.
pub fn wire_input_panel(window: &crate::OperonWindow, state: Rc<RefCell<AppState>>) {
    // Expose initial values from config to the UI
    let app_config = operon_rs::load().ok();

    // 1. Initial Selected Model & reasoning default
    let active_model = app_config
        .as_ref()
        .map(|c| c.provider.model.model_id.clone())
        .unwrap_or_default();
    window.set_selected_model(active_model.into());
    window.set_selected_reasoning("Medium".into()); // Default reasoning level in prompt UI

    // 2. Initial Auto-Approve state
    let auto_approve = app_config
        .as_ref()
        .map(|_c| {
            // Find global policy auto-approve or similar
            false
        })
        .unwrap_or(false);
    window.set_auto_approve_enabled(auto_approve);

    // 3. Initial context usage
    let context_window = app_config
        .as_ref()
        .map(|c| c.provider.model.context_window as i32)
        .unwrap_or(128_000);
    window.set_context_usage(0.0);
    window.set_tokens_used(0);
    window.set_tokens_total(context_window);
    window.set_context_text(context::format_tokens(0, context_window).into());

    // Wire submodules
    attach::wire_attach(window, Rc::clone(&state));
    auto_approve::wire_auto_approve(window, Rc::clone(&state));
    context::wire_context(window, Rc::clone(&state));
    models::wire_models(window, Rc::clone(&state));
    reasoning::wire_reasoning(window, Rc::clone(&state));
    send::wire_send(window, Rc::clone(&state));
    text::wire_text(window, Rc::clone(&state));
    voice::wire_voice(window, Rc::clone(&state));
}
