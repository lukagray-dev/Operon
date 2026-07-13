//! Chat loading skeleton controller.
//!
//! This module wires the `is_loading_session` property on the Slint window
//! so the main content area can show a shimmer skeleton while conversation
//! data is being parsed on the background thread.

/// No active wiring is needed for the loading skeleton — its visibility is
/// driven purely by the `is_loading_session` property set from sidebar.rs.
/// This module exists as the canonical sibling Rust file and can be extended
/// later if the loading state needs additional logic (e.g., timeout fallback).
pub fn wire_loading(_window: &crate::OperonWindow) {
    // The loading skeleton is purely reactive: it reads `is_loading_session`
    // directly from the Slint property tree. The sidebar load handler sets
    // this to true before spawning the async load, and back to false once
    // the messages have been pushed to the UI model.
    //
    // Future extensions:
    // - Add a safety timeout that auto-hides the skeleton after N seconds
    // - Animate a progress indicator if the backend reports load progress
}
