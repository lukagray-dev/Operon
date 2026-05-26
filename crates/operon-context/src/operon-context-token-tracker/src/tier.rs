// tier.rs — EstimationTier enum.
//
// This enum is the single source of truth for *how* a token count was produced.
// Every estimation result carries a tier tag so callers can decide how much to
// trust the number (e.g. only act on Exact values for billing, use Heuristic
// only as a conservative trip-wire for compaction).
//
// Serde is derived so tier values can be stored in session snapshots and
// serialised to JSON for logging or persistence.

use serde::{Deserialize, Serialize};

/// Identifies which tier of the three-tier token estimation strategy produced
/// a given token count.
///
/// Tiers are listed in descending accuracy order:
///
/// | Tier      | When       | Accuracy | Requires          |
/// |-----------|------------|----------|-------------------|
/// | Exact     | Post-call  | Perfect  | API response      |
/// | Bpe       | Pre-call   | ~99%     | `bpe` feature     |
/// | Heuristic | Pre-call   | ~85-90%  | Nothing           |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstimationTier {
    /// Ground truth received from the provider's API response (`usage.input_tokens` /
    /// `usage.output_tokens`). Always prefer this for budget accounting after a call.
    Exact,

    /// BPE tokenisation via `tiktoken` (`cl100k_base` encoding).
    /// Only returned when the crate is compiled with the `bpe` feature flag.
    /// Accurate to ~99% for OpenAI-family models (GPT-3.5/4, DeepSeek, Nvidia NIM).
    Bpe,

    /// Fast content-aware heuristic based on byte length.
    /// Code content: `byte_len / 3`, prose: `byte_len / 4`.
    /// Intentionally overshoots slightly (conservative bias) so compaction
    /// triggers before the hard context limit is hit.
    Heuristic,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_is_copy() {
        // Copy is needed so tiers can be embedded in structs without cloning.
        let t = EstimationTier::Exact;
        let _copy = t; // if Copy wasn't derived this would move and fail below
        let _copy2 = t;
    }

    #[test]
    fn tier_equality() {
        assert_eq!(EstimationTier::Exact, EstimationTier::Exact);
        assert_ne!(EstimationTier::Exact, EstimationTier::Heuristic);
        assert_ne!(EstimationTier::Bpe, EstimationTier::Heuristic);
    }

    #[test]
    fn tier_round_trips_via_serde() {
        // Ensure JSON serialisation/deserialisation is stable so stored session
        // state can be read back correctly after a restart.
        let tiers = [
            EstimationTier::Exact,
            EstimationTier::Bpe,
            EstimationTier::Heuristic,
        ];
        for tier in &tiers {
            let json = serde_json::to_string(tier).expect("serialise");
            let back: EstimationTier = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(*tier, back, "round-trip failed for {tier:?}");
        }
    }

    #[test]
    fn tier_debug_contains_name() {
        // Debug output is used in log lines; confirm it's human-readable.
        assert!(format!("{:?}", EstimationTier::Exact).contains("Exact"));
        assert!(format!("{:?}", EstimationTier::Bpe).contains("Bpe"));
        assert!(format!("{:?}", EstimationTier::Heuristic).contains("Heuristic"));
    }
}
