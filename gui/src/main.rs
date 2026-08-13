//! Binary entry point for the GUI crate.
//!
//! All of the real startup logic lives in the library crate so there is only
//! one place that knows how to build and run the Slint window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    std::env::set_var("SLINT_BACKEND", "winit-skia");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()?;

    let _guard = runtime.enter();

    operon_gui::run()
}
