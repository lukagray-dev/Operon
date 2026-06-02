// session.rs — Cross-turn persistent token state (SessionTokenState).
//
// `SessionTokenState` is the single piece of mutable state that must persist
// across agent turns.  It accumulates the ground-truth exact counts from
// every `UsageRecord` (Tier 1), and also accepts pre-call heuristic or BPE
// estimates when an exact record is not yet available (Tiers 2/3).
//
// Lifetime of a session:
//
//   1. Agent starts → `SessionTokenState::new()`
//   2. Before first call (no exact data yet):
//      → `apply_estimate(estimated_tokens, tier)` stores a pre-call guess.
//   3. After each API call:
//      → `record_turn(&usage_record)` replaces the estimate with ground truth
//         and increments cumulative counters.
//   4. Compaction runs → `reset()` wipes everything for the new condensed
//      context window.
//
// Why `current_context_tokens = record.input_tokens` after a turn?
//
//   The input token count the provider reports is the *full size of the prompt
//   that was sent* — system prompt + entire conversation history + user message.
//   That is exactly what fills the context window, so it is the right number
//   to compare against the budget limit.  Output tokens are added to totals
//   for cost tracking but do not factor into the window-size check.
//
// Serialisation:
//   The struct derives `Serialize`/`Deserialize` so it can be persisted to
//   disk between process restarts (e.g. as part of a session snapshot file).

use serde::{Deserialize, Serialize};

use crate::recorder::UsageRecord;
use crate::tier::EstimationTier;

/// Cross-turn token state for one agent session.
///
/// Persisted between turns and used by `TokenBudget` to decide whether
/// context compaction is needed before the next LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokenState {
    /// Sum of `input_tokens` from all `UsageRecord`s recorded this session.
    /// Useful for per-session cost reporting.
    pub total_input_tokens: usize,

    /// Sum of `output_tokens` from all `UsageRecord`s recorded this session.
    /// Useful for per-session cost reporting.
    pub total_output_tokens: usize,

    /// Token count of the *current* context window — the number of input tokens
    /// the model will receive on the *next* call if nothing changes.
    ///
    /// After `record_turn` this equals `record.input_tokens` (Tier 1 exact).
    /// After `apply_estimate` this is a pre-call estimate (Tier 2 or 3).
    /// This is the value to compare against `TokenBudget::compaction_limit()`.
    pub current_context_tokens: usize,

    /// How many complete turns have been recorded this session.
    pub turn_count: usize,

    /// Which tier provided the most recent `current_context_tokens` value.
    /// Starts as `Heuristic` (most pessimistic) and is set to `Exact` after
    /// the first `record_turn` call.
    pub last_estimation_tier: EstimationTier,
}

impl SessionTokenState {
    /// Create a fresh session with all counters zeroed.
    pub fn new() -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            current_context_tokens: 0,
            turn_count: 0,
            // Default to the weakest tier so the budget is maximally
            // conservative before the first exact record arrives.
            last_estimation_tier: EstimationTier::Heuristic,
        }
    }

    /// Record the exact token usage from a provider API response.
    ///
    /// This is the Tier 1 update path — called after every successful LLM
    /// call with the `usage` block from the API response.
    ///
    /// What it does:
    /// - Adds `record.input_tokens` to `total_input_tokens`.
    /// - Adds `record.output_tokens` to `total_output_tokens`.
    /// - Sets `current_context_tokens = record.input_tokens` (the full prompt
    ///   size that was sent — this is what fills the context window).
    /// - Increments `turn_count`.
    /// - Sets `last_estimation_tier = Exact`.
    ///
    /// Summation uses `saturating_add` — token counts are always bounded by
    /// the model's context window, so a silent clamp is safe.
    pub fn record_turn(&mut self, record: &UsageRecord) {
        // Accumulate into session-level totals for cost/analytics reporting.
        self.total_input_tokens = self.total_input_tokens.saturating_add(record.input_tokens);
        self.total_output_tokens = self
            .total_output_tokens
            .saturating_add(record.output_tokens);

        // current_context_tokens tracks *how full the window is right now*.
        // After a completed turn, that equals the input the model just saw.
        // (The output tokens are generated after the window snapshot, so they
        // don't count toward the next call's context size.)
        self.current_context_tokens = record.input_tokens;

        // We now have ground truth — upgrade the tier tag.
        self.last_estimation_tier = EstimationTier::Exact;

        // Count completed turns for session analytics.
        self.turn_count = self.turn_count.saturating_add(1);
    }

    /// Update `current_context_tokens` from a pre-call estimate.
    ///
    /// Used *only* when an exact `UsageRecord` is not yet available — e.g.
    /// before the very first turn, or after a reset when the compacted context
    /// size is estimated but not yet confirmed by an API response.
    ///
    /// Does **not** touch `total_input_tokens`, `total_output_tokens`, or
    /// `turn_count` because no real API call has happened yet.
    pub fn apply_estimate(&mut self, estimated_tokens: usize, tier: EstimationTier) {
        self.current_context_tokens = estimated_tokens;
        self.last_estimation_tier = tier;
    }

    /// Reset all state for a new session (new task, new conversation, or
    /// post-compaction restart).
    ///
    /// Zeroes every counter and resets the tier to `Heuristic` so the budget
    /// is maximally conservative until the first real API response arrives.
    pub fn reset(&mut self) {
        self.total_input_tokens = 0;
        self.total_output_tokens = 0;
        self.current_context_tokens = 0;
        self.turn_count = 0;
        self.last_estimation_tier = EstimationTier::Heuristic;
    }

    /// Grand total of all tokens consumed this session (input + output).
    ///
    /// Useful for end-of-session cost reporting.  Uses `saturating_add` to
    /// stay within `usize::MAX` rather than panicking.
    pub fn total_tokens(&self) -> usize {
        self.total_input_tokens
            .saturating_add(self.total_output_tokens)
    }
}

impl Default for SessionTokenState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::UsageRecord;

    /// Helper: build a `UsageRecord` with minimal boilerplate.
    fn make_record(input: usize, output: usize) -> UsageRecord {
        UsageRecord {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: None,
            cache_write_tokens: None,
            model: "test-model".to_string(),
            provider: "test-provider".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Initial state
    // -----------------------------------------------------------------------

    #[test]
    fn new_session_is_zeroed() {
        let s = SessionTokenState::new();
        assert_eq!(s.total_input_tokens, 0);
        assert_eq!(s.total_output_tokens, 0);
        assert_eq!(s.current_context_tokens, 0);
        assert_eq!(s.turn_count, 0);
        assert_eq!(s.total_tokens(), 0);
        // Default tier must be Heuristic so we start conservative.
        assert_eq!(s.last_estimation_tier, EstimationTier::Heuristic);
    }

    // -----------------------------------------------------------------------
    // record_turn
    // -----------------------------------------------------------------------

    #[test]
    fn record_turn_increments_cumulative_counters() {
        let mut s = SessionTokenState::new();
        s.record_turn(&make_record(1_000, 250));

        assert_eq!(s.total_input_tokens, 1_000);
        assert_eq!(s.total_output_tokens, 250);
        assert_eq!(s.total_tokens(), 1_250);
        assert_eq!(s.turn_count, 1);
    }

    #[test]
    fn record_turn_sets_current_context_to_input_tokens() {
        // current_context_tokens must track *input* size (the window usage),
        // not total tokens.
        let mut s = SessionTokenState::new();
        s.record_turn(&make_record(5_000, 1_000));
        assert_eq!(s.current_context_tokens, 5_000);
    }

    #[test]
    fn record_turn_upgrades_tier_to_exact() {
        let mut s = SessionTokenState::new();
        assert_eq!(s.last_estimation_tier, EstimationTier::Heuristic);
        s.record_turn(&make_record(100, 50));
        assert_eq!(s.last_estimation_tier, EstimationTier::Exact);
    }

    #[test]
    fn record_turn_multiple_turns_accumulate_correctly() {
        // Simulate a 3-turn conversation.
        let mut s = SessionTokenState::new();
        s.record_turn(&make_record(500, 100)); // turn 1
        s.record_turn(&make_record(700, 150)); // turn 2
        s.record_turn(&make_record(900, 200)); // turn 3

        // Cumulative totals must sum all turns.
        assert_eq!(s.total_input_tokens, 500 + 700 + 900);
        assert_eq!(s.total_output_tokens, 100 + 150 + 200);
        assert_eq!(s.turn_count, 3);

        // current_context_tokens must reflect the LAST turn's input, not cumulative.
        assert_eq!(s.current_context_tokens, 900);
    }

    #[test]
    fn record_turn_saturates_on_overflow() {
        // Providing usize::MAX twice must saturate, not panic or wrap.
        let mut s = SessionTokenState::new();
        s.record_turn(&make_record(usize::MAX, 0));
        s.record_turn(&make_record(1, 0)); // would overflow without saturating_add
        assert_eq!(s.total_input_tokens, usize::MAX);
    }

    // -----------------------------------------------------------------------
    // apply_estimate
    // -----------------------------------------------------------------------

    #[test]
    fn apply_estimate_updates_context_tokens_and_tier() {
        let mut s = SessionTokenState::new();
        s.apply_estimate(3_500, EstimationTier::Bpe);

        assert_eq!(s.current_context_tokens, 3_500);
        assert_eq!(s.last_estimation_tier, EstimationTier::Bpe);
    }

    #[test]
    fn apply_estimate_does_not_affect_cumulative_counters() {
        let mut s = SessionTokenState::new();
        s.apply_estimate(10_000, EstimationTier::Heuristic);

        // No real call happened — cumulative stats must stay at zero.
        assert_eq!(s.total_input_tokens, 0);
        assert_eq!(s.total_output_tokens, 0);
        assert_eq!(s.turn_count, 0);
    }

    #[test]
    fn record_turn_after_estimate_overwrites_context_tokens() {
        // Pre-call estimate followed by the real API response: the exact value
        // must win.
        let mut s = SessionTokenState::new();
        s.apply_estimate(12_000, EstimationTier::Heuristic);
        s.record_turn(&make_record(11_850, 300)); // real value is slightly less

        assert_eq!(s.current_context_tokens, 11_850);
        assert_eq!(s.last_estimation_tier, EstimationTier::Exact);
    }

    // -----------------------------------------------------------------------
    // reset
    // -----------------------------------------------------------------------

    #[test]
    fn reset_zeroes_all_fields() {
        let mut s = SessionTokenState::new();
        // Build up some state.
        s.record_turn(&make_record(5_000, 1_000));
        s.record_turn(&make_record(6_000, 1_200));

        s.reset();

        assert_eq!(s.total_input_tokens, 0);
        assert_eq!(s.total_output_tokens, 0);
        assert_eq!(s.current_context_tokens, 0);
        assert_eq!(s.turn_count, 0);
        // Tier resets to Heuristic (conservative default).
        assert_eq!(s.last_estimation_tier, EstimationTier::Heuristic);
    }

    #[test]
    fn session_is_usable_after_reset() {
        // After reset the struct must behave exactly like a fresh new() instance.
        let mut s = SessionTokenState::new();
        s.record_turn(&make_record(1_000, 200));
        s.reset();
        s.record_turn(&make_record(800, 150));

        assert_eq!(s.total_input_tokens, 800);
        assert_eq!(s.total_output_tokens, 150);
        assert_eq!(s.turn_count, 1);
    }

    // -----------------------------------------------------------------------
    // Serialisation round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn session_state_serialises_and_deserialises() {
        let mut s = SessionTokenState::new();
        s.record_turn(&make_record(2_048, 512));

        let json = serde_json::to_string(&s).expect("serialise");
        let back: SessionTokenState = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.total_input_tokens, s.total_input_tokens);
        assert_eq!(back.total_output_tokens, s.total_output_tokens);
        assert_eq!(back.current_context_tokens, s.current_context_tokens);
        assert_eq!(back.turn_count, s.turn_count);
        assert_eq!(back.last_estimation_tier, s.last_estimation_tier);
    }

    // -----------------------------------------------------------------------
    // Default impl
    // -----------------------------------------------------------------------

    #[test]
    fn default_matches_new() {
        let a = SessionTokenState::new();
        let b = SessionTokenState::default();
        assert_eq!(a.total_tokens(), b.total_tokens());
        assert_eq!(a.turn_count, b.turn_count);
    }
}
