//! # operon-context-normalize-stream
//!
//! Canonical streaming event normalization for ten major LLM providers.
//!
//! ## What this crate does
//!
//! Exactly one job:
//!
//! ```text
//! provider SSE/NDJSON line  ->  canonical StreamEvent(s)  ->  StreamAssembler output
//! ```
//!
//! The crate is sync-only and push-based:
//! - It receives one already-split line at a time.
//! - It does not perform HTTP I/O.
//! - It does not depend on async runtimes.
//!
//! ## Quick example
//!
//! ```rust
//! use operon_context_normalize_stream::{
//!     parse_line, new_assembler, AssemblerOutput, Provider,
//! };
//!
//! let provider = Provider::OpenAI;
//! let mut assembler = new_assembler(&provider);
//!
//! let lines = [
//!     r#"{"choices":[{"delta":{"content":"Hello "},"finish_reason":null}]}"#,
//!     r#"{"choices":[{"delta":{"content":"world"},"finish_reason":"stop"}]}"#,
//! ];
//!
//! for line in lines {
//!     let events = parse_line(line, &provider).unwrap();
//!     for event in events {
//!         let output = assembler.push(event).unwrap();
//!         if let AssemblerOutput::Text(text) = output {
//!             assert!(!text.is_empty());
//!         }
//!     }
//! }
//!
//! let ended_outputs = assembler.finish().unwrap();
//! assert!(matches!(ended_outputs[0], AssemblerOutput::StreamEnded { .. }));
//! ```

pub mod assembler;
pub mod error;
pub mod normalize;
pub mod provider;
pub mod types;

// `Provider` is the single authoritative enum from operon-providers.
// We import it directly here (not through normalize-tools) so normalize-stream's
// dependency on Provider is explicit and matches the pattern in the other three
// normalize crates.
pub use assembler::StreamAssembler;
pub use error::{Result, StreamNormalizeError};
pub use normalize::{new_assembler, parse_line};
pub use operon_providers::Provider;
pub use types::{AssemblerOutput, StreamEvent};
