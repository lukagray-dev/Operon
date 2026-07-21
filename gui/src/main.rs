//! Binary entry point for the GUI crate.
//!
//! All of the real startup logic lives in the library crate so there is only
//! one place that knows how to build and run the Slint window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("SLINT_BACKEND", "winit-skia");
    operon_gui::run()
}
