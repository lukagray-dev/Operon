//! Library entry point for the GUI crate.
//!
//! The compiled Slint components are included here so both the binary target
//! and the integration tests can work against the same generated UI types.

slint::include_modules!();

pub mod app;
pub mod settings;
pub mod state;
pub mod window;

#[path = "left-sidebar/mod.rs"]
pub mod left_sidebar;

#[path = "main-content/main-content.rs"]
pub mod main_content;

#[path = "right-sidebar/mod.rs"]
pub mod right_sidebar;

pub mod executor;

/// Starts the Operon GUI application.
///
/// Keeping the public run function in the library lets the binary stay tiny and
/// lets tests reuse the exact same boot path.
pub fn run() -> anyhow::Result<()> {
    app::run()
}
