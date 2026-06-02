//! # operon-events
//!
//! Pure-types crate providing the event bus types for the Operon agent loop.
//!
//! Two types are provided:
//!
//! - [`SessionEvent`] — emitted **by** the runner **to** the UI over an outbound `mpsc` channel.
//! - [`SessionCommand`] — sent **by** the UI **into** the runner over an inbound `mpsc` channel.
//!
//! ## Design constraints
//!
//! - **No async, no I/O, no tokio.** Only plain Rust types with `serde` derives.
//!   Any crate in the workspace can depend on this without pulling in a runtime.
//!
//! - **No tools dependency.** Tool call IDs are plain `String` values here,
//!   deliberately avoiding a dependency on `operon-context-normalize-tools`.
//!
//! - **Serializable.** All types derive [`serde::Serialize`] / [`serde::Deserialize`]
//!   so events can be logged, replayed, or transmitted over a network.
//!
//! ## Channel setup
//!
//! ```rust
//! use operon_events::{SessionEvent, SessionCommand};
//!
//! // Outbound: runner → UI
//! let event = SessionEvent::TextDelta { text: "Hello".to_string() };
//! match event {
//!     SessionEvent::TextDelta { text }     => print!("{text}"),
//!     SessionEvent::TokenUsageUpdated { context_total, .. } => {
//!         eprintln!("Tokens used: {context_total}");
//!     }
//!     SessionEvent::ContextUsageUpdated { utilization, .. } => {
//!         eprintln!("Context usage: {:.0}%", utilization * 100.0);
//!     }
//!     SessionEvent::Done                   => println!("\nDone."),
//!     SessionEvent::Error { message }      => eprintln!("Error: {message}"),
//!     _ => {}
//! }
//!
//! // Inbound: UI → runner
//! let cmd = SessionCommand::Cancel;
//! match cmd {
//!     SessionCommand::Cancel        => { /* stop the loop */ }
//!     SessionCommand::Approve { id } => { /* unblock Ask-mode tool */ }
//!     SessionCommand::Deny { id }    => { /* reject Ask-mode tool */ }
//! }
//! ```

// Declare submodules — session.rs holds both the event and command types.
pub mod session;

// ─────────────────────────────────────────────────────────────────────────────
// Public re-exports
// ─────────────────────────────────────────────────────────────────────────────

/// All events emitted by the session runner to the UI.
///
/// Consumers can write `use operon_events::SessionEvent;` rather than
/// `use operon_events::session::SessionEvent;`.
pub use session::SessionEvent;

/// Commands sent from the UI into the session runner.
///
/// The UI holds an `mpsc::Sender<SessionCommand>` and the runner holds the
/// corresponding `mpsc::Receiver<SessionCommand>`.
pub use session::SessionCommand;
