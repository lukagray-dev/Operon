// budget.rs — Context-window budget and compaction-threshold logic.
//
// `TokenBudget` is a small, immutable value type that encodes two configuration
// parameters for a session:
//
//   1. `context_window`       — the hard token limit of the active model
//                               (e.g. 200_000 for Claude Sonnet, 128_000 for GPT-4o).
//   2. `compaction_threshold` — the fraction of the context window at which
//                               compaction should be triggered (default 0.90 = 90%).
//
// It exposes derived queries (`compaction_limit`, `remaining`, `should_compact`,
// `utilization`) that the session layer calls after every turn to decide whether
// to kick off a context-compaction run before the next LLM call.
//
// Why trigger at 90% rather than 100%?
//   The model needs headroom for the *next* response.  If you wait until you
//   are at 100% the next generation will be truncated or rejected.  90% is a
//   conservative default; callers can set it lower for safety or higher for
//   maximum context utilisation.
//
// Arithmetic notes:
//   - `compaction_limit` is computed in f64 to avoid integer truncation errors
//     on large windows (e.g. 1_000_000 * 0.90 → 900_000 exactly).
//   - `utilization` is clamped to [0.0, 1.0] so callers don't need to guard
//     against values > 1.0 when the estimate overshoots slightly.

use crate::error::{Result, TokenTrackerError};

/// Immutable budget configuration for one model session.
///
/// Construct once with `TokenBudget::with_window(n)` (90% threshold) or
/// `TokenBudget::new(n, threshold)` for a custom threshold, then pass it
/// around cheaply (it's two words on the stack).
#[derive(Debug, Clone, Copy)]
pub struct TokenBudget {
    /// Total context window size of the active model in tokens.
    context_window: usize,

    /// Fraction of `context_window` at which compaction is triggered.
    /// Valid range: `(0.0, 1.0]`.
    compaction_threshold: f32,
}

impl TokenBudget {
    /// Create a budget with an explicit compaction threshold.
    ///
    /// # Errors
    ///
    /// - [`TokenTrackerError::InvalidContextWindow`] if `context_window == 0`.
    /// - [`TokenTrackerError::InvalidThreshold`] if `compaction_threshold` is
    ///   not in the open-closed range `(0.0, 1.0]`.  NaN and infinity are also
    ///   rejected.
    pub fn new(context_window: usize, compaction_threshold: f32) -> Result<Self> {
        // A zero-sized window makes all the derived maths meaningless.
        if context_window == 0 {
            return Err(TokenTrackerError::InvalidContextWindow);
        }

        // Use a positive-guard so NaN / infinity also fail (comparisons
        // against NaN always return false, so `!(x > 0.0 && x <= 1.0)`
        // catches NaN correctly).
        if !(compaction_threshold > 0.0 && compaction_threshold <= 1.0) {
            return Err(TokenTrackerError::InvalidThreshold(compaction_threshold));
        }

        Ok(Self {
            context_window,
            compaction_threshold,
        })
    }

    /// Create a budget with the default 90% compaction threshold.
    ///
    /// This is the recommended constructor for most use-cases: it gives the
    /// model ~10% headroom for its next output before the window is full.
    ///
    /// # Errors
    ///
    /// Returns [`TokenTrackerError::InvalidContextWindow`] if `context_window == 0`.
    pub fn with_window(context_window: usize) -> Result<Self> {
        Self::new(context_window, 0.90)
    }

    /// The configured total context window size in tokens.
    pub fn context_window(&self) -> usize {
        self.context_window
    }

    /// The configured compaction threshold as a fraction in `(0.0, 1.0]`.
    pub fn compaction_threshold(&self) -> f32 {
        self.compaction_threshold
    }

    /// Absolute token count at which compaction should be triggered.
    ///
    /// `compaction_limit = floor(context_window × compaction_threshold)`
    ///
    /// Computed in `f64` so that large context windows (millions of tokens)
    /// do not lose precision from f32 rounding.
    pub fn compaction_limit(&self) -> usize {
        // Cast to f64 for full precision on large windows, then round to nearest
        // integer to avoid floating-point representation truncation errors (e.g. 0.90f32
        // being slightly less than 0.90f64). The result is always <= context_window
        // because threshold <= 1.0.
        (self.context_window as f64 * self.compaction_threshold as f64).round() as usize
    }

    /// How many more tokens can be added before the compaction limit is hit.
    ///
    /// Returns `0` if `current_tokens` already equals or exceeds the limit
    /// (saturating subtraction, never negative).
    pub fn remaining(&self, current_tokens: usize) -> usize {
        self.compaction_limit().saturating_sub(current_tokens)
    }

    /// Returns `true` when `current_tokens` has reached or exceeded the
    /// compaction limit and a context-compaction run should be started before
    /// the next LLM call.
    pub fn should_compact(&self, current_tokens: usize) -> bool {
        current_tokens >= self.compaction_limit()
    }

    /// Fraction of the context window consumed: `current_tokens / context_window`.
    ///
    /// Clamped to `[0.0, 1.0]` so callers can treat it as a percentage
    /// without additional guards.  Computed in f64 to avoid precision loss on
    /// large windows.
    pub fn utilization(&self, current_tokens: usize) -> f32 {
        // context_window is guaranteed > 0 by the constructor, so no
        // division-by-zero risk here.
        let ratio = current_tokens as f64 / self.context_window as f64;
        // min(1.0) clamps; the cast to f32 loses some precision but that is
        // acceptable for a display/logging metric.
        ratio.min(1.0) as f32
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Constructor validation
    // -----------------------------------------------------------------------

    #[test]
    fn new_rejects_zero_context_window() {
        let err = TokenBudget::new(0, 0.9).expect_err("should reject window=0");
        assert!(
            matches!(err, TokenTrackerError::InvalidContextWindow),
            "unexpected error variant: {err:?}"
        );
    }

    #[test]
    fn new_rejects_zero_threshold() {
        let err = TokenBudget::new(200_000, 0.0).expect_err("should reject threshold=0.0");
        assert!(matches!(err, TokenTrackerError::InvalidThreshold(_)));
    }

    #[test]
    fn new_rejects_threshold_above_one() {
        let err = TokenBudget::new(200_000, 1.1).expect_err("should reject threshold=1.1");
        assert!(matches!(err, TokenTrackerError::InvalidThreshold(_)));
    }

    #[test]
    fn new_rejects_nan_threshold() {
        let err = TokenBudget::new(200_000, f32::NAN).expect_err("NaN threshold must be rejected");
        assert!(matches!(err, TokenTrackerError::InvalidThreshold(_)));
    }

    #[test]
    fn new_rejects_infinite_threshold() {
        let err =
            TokenBudget::new(200_000, f32::INFINITY).expect_err("inf threshold must be rejected");
        assert!(matches!(err, TokenTrackerError::InvalidThreshold(_)));
    }

    #[test]
    fn new_accepts_threshold_of_exactly_one() {
        // threshold = 1.0 is valid (compact only when the window is full).
        let b = TokenBudget::new(100_000, 1.0).expect("1.0 is a valid threshold");
        assert_eq!(b.compaction_threshold(), 1.0);
    }

    #[test]
    fn with_window_uses_90_percent_threshold() {
        let b = TokenBudget::with_window(200_000).expect("valid window");
        assert!((b.compaction_threshold() - 0.90).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // Derived calculations
    // -----------------------------------------------------------------------

    #[test]
    fn compaction_limit_is_threshold_fraction_of_window() {
        let b = TokenBudget::new(200_000, 0.90).unwrap();
        // 200_000 * 0.90 = 180_000
        assert_eq!(b.compaction_limit(), 180_000);
    }

    #[test]
    fn should_compact_false_below_threshold() {
        let b = TokenBudget::with_window(200_000).unwrap();
        // 180_000 is the limit; 179_999 is one token under → should not compact.
        assert!(!b.should_compact(179_999));
    }

    #[test]
    fn should_compact_true_at_threshold() {
        let b = TokenBudget::with_window(200_000).unwrap();
        // At exactly the limit → should compact.
        assert!(b.should_compact(180_000));
    }

    #[test]
    fn should_compact_true_above_threshold() {
        let b = TokenBudget::with_window(200_000).unwrap();
        assert!(b.should_compact(180_500));
    }

    #[test]
    fn remaining_returns_headroom_before_limit() {
        let b = TokenBudget::with_window(200_000).unwrap();
        // limit = 180_000; current = 150_000 → 30_000 remaining.
        assert_eq!(b.remaining(150_000), 30_000);
    }

    #[test]
    fn remaining_returns_zero_when_at_or_over_limit() {
        let b = TokenBudget::with_window(200_000).unwrap();
        assert_eq!(b.remaining(180_000), 0); // at limit
        assert_eq!(b.remaining(190_000), 0); // over limit
    }

    #[test]
    fn utilization_is_fraction_of_context_window() {
        let b = TokenBudget::with_window(100_000).unwrap();
        let u = b.utilization(50_000);
        // 50_000 / 100_000 = 0.5
        assert!((u - 0.5).abs() < 1e-5, "expected 0.5, got {u}");
    }

    #[test]
    fn utilization_clamps_to_one_when_over_window() {
        let b = TokenBudget::with_window(100_000).unwrap();
        let u = b.utilization(999_999);
        assert!((u - 1.0).abs() < 1e-5, "expected 1.0 (clamped), got {u}");
    }

    #[test]
    fn utilization_zero_when_no_tokens_used() {
        let b = TokenBudget::with_window(100_000).unwrap();
        assert!((b.utilization(0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn budget_is_copy() {
        // Copy allows cheap passing without clone() calls.
        let b = TokenBudget::with_window(128_000).unwrap();
        let _copy1 = b;
        let _copy2 = b; // would fail to compile if not Copy
    }
}
