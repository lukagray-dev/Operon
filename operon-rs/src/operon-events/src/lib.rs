//! # operon-events
//!
//! Pure-types crate providing the [`SessionEvent`] enum — the event bus type
//! emitted by `operon-session`'s agent loop over an async mpsc channel.
//!
//! ## Design constraints
//!
//! - **No async, no I/O, no tokio.** This crate contains only plain Rust types
//!   with `serde` derives. Any crate in the workspace can depend on it without
//!   transitively pulling in a runtime.
//!
//! - **No tools dependency.** Tool call IDs are plain `String` values here,
//!   deliberately avoiding a dependency on `operon-context-normalize-tools`.
//!   This keeps the crate minimal and dependency-free.
//!
//! - **Serializable.** All types derive [`serde::Serialize`] and
//!   [`serde::Deserialize`] so events can be logged, replayed, or transmitted
//!   over a network without extra scaffolding.
//!
//! ## Usage
//!
//! ```rust
//! use operon_events::SessionEvent;
//!
//! // Typically received from an mpsc::Receiver<SessionEvent>:
//! let event = SessionEvent::TextDelta { text: "Hello".to_string() };
//! match event {
//!     SessionEvent::TextDelta { text } => print!("{text}"),
//!     SessionEvent::Done => println!("\nSession complete."),
//!     SessionEvent::Error { message } => eprintln!("Error: {message}"),
//!     _ => {}
//! }
//! ```

// Declare submodules — session.rs holds the main event type.
pub mod session;

// Re-export the primary type at the crate root for ergonomic imports.
// Consumers can write `use operon_events::SessionEvent;` rather than
// `use operon_events::session::SessionEvent;`.
pub use session::SessionEvent;
