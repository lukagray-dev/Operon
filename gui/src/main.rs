//! Binary entry point for the GUI crate.
//!
//! All of the real startup logic lives in the library crate so there is only
//! one place that knows how to build and run the Slint window.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    operon_gui::run()
}
