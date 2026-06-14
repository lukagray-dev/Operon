//! # operon-session
//!
//! Agent loop, provider HTTP, SSE stream consumption, tool dispatch coordination,
//! context compaction triggering, and session persistence for the Operon system.
//!
//! ## Architecture
//!
//! `operon-session` sits between the frontend (TUI, GUI) and the context/tools
//! pipeline. It is the only crate that orchestrates the full agentic loop.
//!
//! ```text
//! Frontend
//!   └── SessionRunner::new(config, event_tx, cmd_rx)
//!         └── runner.run(user_message)
//!               ├── SnapshotBuilder::build()       → system prompt
//!               ├── sanitize()                     → clean messages
//!               ├── build_request()                → provider JSON body
//!               ├── send_streaming()               → SSE stream → SessionEvents
//!               ├── Dispatcher::dispatch()         → tool results
//!               ├── compact()                      → condensed history
//!               └── SessionStore::save_turn()      → SQLite persistence
//! ```
//!
//! ## Key types
//!
//! | Type                | Purpose                                          |
//! |---------------------|--------------------------------------------------|
//! | [`SessionRunner`]   | Owns all session state; drives the agent loop    |
//! | [`SessionConfig`]   | All runtime parameters (provider, model, paths)  |
//! | [`SessionError`]    | Unified error type for all session operations    |
//! | [`LifecycleState`]  | State machine for run/pause/done/failed          |
//!
//! Events are emitted on an `mpsc::Sender<SessionEvent>` (from `operon-events`).
//! The frontend receives them and renders accordingly.

// ── Module declarations ───────────────────────────────────────────────────────
// Each file corresponds to one logical subsystem concern.

/// Session configuration — all runtime parameters for a `SessionRunner`.
pub mod config;

/// Session-level error type covering all failure modes in the loop.
pub mod error;

/// HTTP request builder and SSE stream consumer.
pub mod http;

/// Lifecycle state machine (Idle → Running → Done/Failed).
pub mod lifecycle;

/// Provider request body construction from canonical types.
pub mod request;

/// The agent loop — `SessionRunner` lives here.
pub mod runner;

/// Module containing session submodules.
pub mod session;

/// SQLite-backed turn persistence.
pub mod store;

// ── Public re-exports ─────────────────────────────────────────────────────────
// These are the types consumers import from `operon_session::*`.

/// The primary session configuration type.
pub use config::SessionConfig;

/// The unified session error type.
pub use error::SessionError;

/// The lifecycle state machine enum.
pub use lifecycle::LifecycleState;

/// The agent loop runner — the main entry point for consumers.
pub use runner::SessionRunner;
