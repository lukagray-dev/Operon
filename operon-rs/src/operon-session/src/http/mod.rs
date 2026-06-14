// mod.rs — Exposes the provider request streaming functionality.
//
// Hey friend! This is the entry point for the http module.
// We declare our submodules `headers` and `stream`, and then re-export
// `StreamResult` and `send_streaming` for use by the rest of the crate.

pub mod detector;
pub mod headers;
pub mod stream;

pub use stream::{StreamResult, send_streaming};
