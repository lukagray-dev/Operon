//! Library entry point for the GUI crate.
//!
//! The compiled Slint components are included here so both the binary target
//! and the integration tests can work against the same generated UI types.

slint::include_modules!();

pub mod app;
pub mod state;
pub mod window;
pub mod settings;

#[path = "left-sidebar/mod.rs"]
pub mod left_sidebar;

#[path = "main-content/main-content.rs"]
pub mod main_content;

pub mod executor;

/// Starts the Operon GUI application.
///
/// Keeping the public run function in the library lets the binary stay tiny and
/// lets tests reuse the exact same boot path.
pub fn run() -> anyhow::Result<()> {
    app::run()
}
