//! Window-specific helpers for the custom Slint titlebar.
//!
//! The `.slint` files own the actual UI tree, while this Rust module handles
//! the side effects behind the buttons and menu items.

pub mod action;
pub mod autostart;
pub mod menu;
pub mod navigation;
pub mod notification;
pub mod startup;
pub mod titlebar;
pub mod tray;
