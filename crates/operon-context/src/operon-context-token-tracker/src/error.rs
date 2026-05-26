// error.rs — Crate-level error types for operon-context-token-tracker.
//
// All errors the public API can surface are defined here as a single flat enum.
// We use `thiserror` to auto-generate the Display/Error impls from the #[error]
// annotations, keeping the code concise and consistent.
//
// The `Result<T>` alias at the bottom lets every other module in this crate write
// `Result<T>` instead of `std::result::Result<T, TokenTrackerError>`.

use thiserror::Error;

/// All errors that the token-tracker crate can return.
///
/// The variants are intentionally narrow so callers can match precisely
/// without having to inspect an opaque error string.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TokenTrackerError {
    /// Returned when a `TokenBudget` is constructed with a context window of 0.
    /// A zero-sized window makes all budget math nonsensical.
    #[error("context window size must be > 0")]
    InvalidContextWindow,

    /// Returned when the compaction threshold is outside the valid range (0.0, 1.0].
    /// A threshold of exactly 0.0 would trigger compaction immediately on every call,
    /// and anything above 1.0 would never trigger, both of which are logic errors.
    #[error("compaction threshold must be between 0.0 and 1.0, got {0}")]
    InvalidThreshold(f32),

    /// Returned when a token counter would overflow a `usize`.
    /// In practice this requires an astronomically large session, but we guard it
    /// in places where overflow would cause an incorrect compaction decision.
    #[error("token count overflow")]
    Overflow,
}

/// Convenience `Result` alias so every module can write `Result<T>` instead of
/// `std::result::Result<T, TokenTrackerError>`.
pub type Result<T> = std::result::Result<T, TokenTrackerError>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_context_window_display() {
        // Verify the human-readable message so downstream error logs are clear.
        let err = TokenTrackerError::InvalidContextWindow;
        assert_eq!(err.to_string(), "context window size must be > 0");
    }

    #[test]
    fn invalid_threshold_display_includes_value() {
        // The bad value should appear in the message so it is easy to debug.
        let err = TokenTrackerError::InvalidThreshold(1.5);
        assert!(
            err.to_string().contains("1.5"),
            "expected threshold value in message, got: {err}"
        );
    }

    #[test]
    fn overflow_display() {
        let err = TokenTrackerError::Overflow;
        assert_eq!(err.to_string(), "token count overflow");
    }

    #[test]
    fn result_alias_ok_path() {
        // Confirm the type alias resolves to the right Result type.
        let r: Result<u32> = Ok(42);
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn result_alias_err_path() {
        let r: Result<u32> = Err(TokenTrackerError::Overflow);
        assert!(matches!(r, Err(TokenTrackerError::Overflow)));
    }
}
