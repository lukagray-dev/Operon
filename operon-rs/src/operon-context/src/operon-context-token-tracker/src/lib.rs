//! # operon-context-token-tracker
//!
//! Token estimation and budget management for LLM context window control.
//!
//! Part of the [Operon](https://github.com/lukagray-dev/operon) project but
//! independently usable by any Rust developer building LLM applications.
//!
//! ---
//!
//! ## Three-Tier Token Counting Strategy
//!
//! | Tier      | When       | Accuracy | Requires      |
//! |-----------|------------|----------|---------------|
//! | Exact     | Post-call  | Perfect  | API response  |
//! | BPE       | Pre-call   | ~99%     | `bpe` feature |
//! | Heuristic | Pre-call   | ~85-90%  | Nothing       |
//!
//! **Tier 1 — Exact**: Record actual counts from the provider API response
//! (`usage.input_tokens`, `usage.output_tokens`).  Ground truth, used to
//! correct session state after every turn.
//!
//! **Tier 2 — BPE** (`bpe` feature): `tiktoken` `cl100k_base` encoding.
//! ~99% accurate for OpenAI-family models (GPT-3.5/4, DeepSeek, Nvidia NIM).
//!
//! **Tier 3 — Heuristic** (always available, zero deps): content-aware
//! byte-length estimation.  Code: `byte_len / 3`, prose: `byte_len / 4`.
//! Intentionally overshoots slightly so compaction triggers before the hard
//! provider limit.
//!
//! ---
//!
//! ## Quick Start
//!
//! ```rust
//! use operon_context_token_tracker::{
//!     TokenEstimator, TokenBudget, SessionTokenState, UsageRecord, TokenRecorder,
//! };
//!
//! // --- Pre-call estimation ---
//! let (count, tier) = TokenEstimator::estimate("Hello, world!");
//! println!("Estimated {count} tokens via {tier:?}");
//!
//! // --- Budget check ---
//! let budget = TokenBudget::with_window(200_000).unwrap();
//! if budget.should_compact(180_500) {
//!     // kick off context compaction before the next LLM call
//! }
//!
//! // --- Post-call: record exact usage from the API response ---
//! let mut session = SessionTokenState::new();
//! let record = UsageRecord {
//!     input_tokens:       1_024,
//!     output_tokens:      256,
//!     cache_read_tokens:  None,
//!     cache_write_tokens: None,
//!     model:    "claude-sonnet-4-5".into(),
//!     provider: "anthropic".into(),
//! };
//! session.record_turn(&record);
//!
//! // --- Multi-message pre-call estimate ---
//! let history = vec!["System prompt here.", "User: tell me about Rust.", "Assistant: Rust is ..."];
//! let (total, _tier) = TokenEstimator::estimate_messages(
//!     &history.iter().map(|s| *s).collect::<Vec<_>>()
//! );
//! println!("Conversation so far: ~{total} tokens");
//! ```

// Declare all internal modules.
// They are `pub(crate)` by default; only the items re-exported below are public.
mod budget;
mod error;
mod estimator;
mod recorder;
mod session;
mod tier;

// ---------------------------------------------------------------------------
// Public API re-exports
// ---------------------------------------------------------------------------

/// Error type and Result alias for all fallible operations in this crate.
pub use error::{Result, TokenTrackerError};

/// Identifies which tier produced a given token estimate.
pub use tier::EstimationTier;

/// Stateless pre-call token estimator (Tiers 2 and 3).
pub use estimator::TokenEstimator;

/// A single turn's exact token usage from the provider API.
pub use recorder::UsageRecord;

/// Accumulates per-turn `UsageRecord`s and exposes aggregate statistics.
pub use recorder::TokenRecorder;

/// Context-window budget and compaction-threshold logic.
pub use budget::TokenBudget;

/// Cross-turn persistent session token state.
pub use session::SessionTokenState;
