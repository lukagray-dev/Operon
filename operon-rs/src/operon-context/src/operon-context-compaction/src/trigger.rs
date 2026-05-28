//! Pure threshold gate for deciding whether compaction should run.
//!
//! The caller extracts token usage from `SessionTokenState` and passes the
//! number here. Keeping this module independent avoids coupling compaction to
//! the token tracker crate.

use crate::CompactionConfig;

/// Returns true when current token usage meets or exceeds the configured
/// compaction threshold.
pub fn should_compact(used_tokens: usize, config: &CompactionConfig) -> bool {
    let threshold = (config.context_window as f32 * config.threshold_pct) as usize;
    used_tokens >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(context_window: usize, threshold_pct: f32) -> CompactionConfig {
        CompactionConfig {
            preserved_turns: 2,
            threshold_pct,
            context_window,
        }
    }

    #[test]
    fn threshold_exactly_at_boundary_fires() {
        let config = config(1_000, 0.90);

        assert!(should_compact(900, &config));
    }

    #[test]
    fn threshold_one_below_boundary_does_not_fire() {
        let config = config(1_000, 0.90);

        assert!(!should_compact(899, &config));
    }

    #[test]
    fn threshold_one_point_zero_only_fires_at_full_capacity() {
        let config = config(1_000, 1.0);

        assert!(!should_compact(999, &config));
        assert!(should_compact(1_000, &config));
    }
}
