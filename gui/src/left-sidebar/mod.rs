//! Left sidebar controller orchestration.
//!
//! This module registers the sidebar view components and sets up the coordination
//! logic for displaying project and standalone chat lists.

pub mod sidebar;
pub mod chats;
pub mod projects;
pub mod search;

use std::cell::RefCell;
use std::rc::Rc;
use crate::state::AppState;

/// Setup and wire the left sidebar view actions and data models.
pub fn wire_left_sidebar(
    window: &crate::OperonWindow,
    state: Rc<RefCell<AppState>>,
) {
    sidebar::wire_sidebar(window, state);
}
