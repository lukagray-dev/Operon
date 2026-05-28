//! Error types for context compaction.
//!
//! This crate deliberately keeps its error surface small. The caller owns the
//! LLM request implementation, so compaction only reports control-flow reasons
//! to skip work, client failures, and JSON serialization failures.

/// Errors returned by the compaction pipeline.
#[derive(Debug, thiserror::Error)]
pub enum CompactionError {
    /// Returned when `compact` is called before the configured token threshold.
    #[error("Compaction threshold not reached — caller should not have called compact()")]
    ThresholdNotReached,

    /// Returned when there is no older history to replace with a summary.
    #[error("Insufficient message history to compact (compactable portion is empty)")]
    InsufficientHistory,

    /// Wraps failures produced by the caller-provided LLM summarization client.
    #[error("Summarization client returned an error: {0}")]
    ClientError(String),

    /// Wraps JSON rendering failures while preparing prompts or estimates.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_error_message_is_actionable() {
        let message = CompactionError::ThresholdNotReached.to_string();
        assert!(
            message.contains("caller should not have called compact()"),
            "threshold error should explain that this is a caller-side gate"
        );
    }

    #[test]
    fn client_error_includes_inner_message() {
        let err = CompactionError::ClientError("provider timeout".to_string());
        assert!(
            err.to_string().contains("provider timeout"),
            "client error should include the caller-provided failure text"
        );
    }
}
