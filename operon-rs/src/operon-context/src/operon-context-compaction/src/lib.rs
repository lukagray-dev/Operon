//! # operon-context-compaction
//!
//! Token-threshold driven context compaction for Operon conversations.
//!
//! This crate replaces older conversation history with an LLM-generated summary
//! while preserving recent complete turns verbatim. It does not own provider
//! HTTP calls; callers inject a [`CompactionClient`] implementation so runtime
//! code can choose any LLM provider and tests can stay deterministic.

mod client;
mod compactor;
mod error;
mod prompt;
mod splitter;
mod trigger;

use operon_context_normalize_messages::ConversationMessage;
use serde::{Deserialize, Serialize};

/// Re-export the Anthropic HTTP client when the `http-client` feature is enabled.
/// The session crate depends on this feature so it can construct the client directly.
#[cfg(feature = "http-client")]
pub use client::AnthropicCompactionClient;
pub use client::CompactionClient;
#[cfg(any(test, feature = "test-utils"))]
pub use client::MockCompactionClient;
pub use compactor::compact;
pub use error::CompactionError;
pub use prompt::build_prompt;
pub use splitter::{split_messages, SplitMessages};
pub use trigger::should_compact;

/// Runtime configuration for context compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Last N complete user turns kept verbatim after compaction.
    pub preserved_turns: usize,
    /// Compact when token usage reaches this fraction of `context_window`.
    pub threshold_pct: f32,
    /// Total context window size in tokens for the active model.
    pub context_window: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            preserved_turns: 2,
            threshold_pct: 0.90,
            context_window: 200_000,
        }
    }
}

/// Result returned after a successful compaction run.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Rebuilt message array: fresh system snapshot, summary, then preserved turns.
    pub messages: Vec<ConversationMessage>,
    /// Summary text returned by the client, intended for logging or debugging.
    pub summary: String,
    /// Token usage before compaction, supplied by the caller.
    pub tokens_before: usize,
    /// Heuristic token estimate for the rebuilt message array.
    pub tokens_after: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_matches_documented_values() {
        let config = CompactionConfig::default();

        assert_eq!(config.preserved_turns, 2);
        assert!((config.threshold_pct - 0.90).abs() < f32::EPSILON);
        assert_eq!(config.context_window, 200_000);
    }
}
