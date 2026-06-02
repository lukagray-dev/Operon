// estimator.rs — Stateless pre-call token estimation (Tier 2: BPE, Tier 3: Heuristic).
//
// `TokenEstimator` is a zero-sized unit struct — all methods are static/associated
// functions because there is no per-instance state. The BPE encoder, when compiled
// in, is stored in a process-global OnceLock so it pays the initialisation cost
// exactly once across all calls, threads, and crate users.
//
// Dispatch logic (compile-time, no runtime branch):
//   - `bpe` feature ON  → BPE path via tiktoken cl100k_base → EstimationTier::Bpe
//   - `bpe` feature OFF → Heuristic path                   → EstimationTier::Heuristic
//
// The heuristic overshoots deliberately (conservative bias) so context compaction
// trips before the hard provider limit, not after it.

use crate::tier::EstimationTier;

// Per-message overhead to match OpenAI's chat-completion token accounting:
// each message adds 3 tokens for role/separator plus 1 reply-priming token.
const CHAT_MESSAGE_OVERHEAD: usize = 4;

// Characters that, if dense enough, classify text as code-like.
// We count these relative to total chars; >=15% → code.
const CODE_CHARS: &[char] = &['{', '}', '(', ')', ';', '[', ']', '<', '>', '/'];

/// Stateless token estimator.  All methods are associated functions — no
/// instance is needed.  Just call `TokenEstimator::estimate(text)`.
#[derive(Debug, Clone, Copy)]
pub struct TokenEstimator;

impl TokenEstimator {
    /// Estimate the token count for a single `text` string.
    ///
    /// Returns `(token_count, tier_used)` so callers know exactly how the
    /// count was produced and can decide how much to trust it.
    ///
    /// - With the `bpe` feature: uses `tiktoken` (`cl100k_base`) for ~99%
    ///   accuracy against OpenAI-family models.
    /// - Without the `bpe` feature: uses the heuristic (byte-length / 3 for
    ///   code, / 4 for prose). Always >= 1.
    pub fn estimate(text: &str) -> (usize, EstimationTier) {
        // ----------------------------------------------------------------
        // Tier 2 — BPE path (only compiled when `bpe` feature is enabled).
        // We use a process-wide OnceLock so the encoder is loaded once and
        // then reused for free on every subsequent call.
        // ----------------------------------------------------------------
        #[cfg(feature = "bpe")]
        {
            use std::sync::OnceLock;

            // SAFETY: OnceLock guarantees exactly one initialisation even
            // under concurrent calls from multiple threads.
            static ENCODER: OnceLock<&'static tiktoken::CoreBpe> = OnceLock::new();

            let encoder = ENCODER.get_or_init(|| {
                // cl100k_base is intentionally used for ALL providers — it is
                // the industry-standard BPE approximation for any OpenAI-family
                // tokeniser (GPT-3.5/4, DeepSeek, Nvidia NIM, etc.).
                tiktoken::get_encoding("cl100k_base")
                    .expect("cl100k_base encoding must be available")
            });

            // encode_with_special_tokens counts <|...|> tokens too, giving a
            // count that better matches what the API will actually bill.
            let count = encoder.encode_with_special_tokens(text).len().max(1);
            return (count, EstimationTier::Bpe);
        }

        // ----------------------------------------------------------------
        // Tier 3 — Heuristic path (always compiled, zero extra deps).
        // ----------------------------------------------------------------
        #[cfg(not(feature = "bpe"))]
        {
            (Self::heuristic_estimate(text), EstimationTier::Heuristic)
        }
    }

    /// Estimate total tokens for a slice of message strings (e.g. a full
    /// conversation thread).
    ///
    /// Adds `4` tokens per message for chat-format overhead — this matches
    /// OpenAI's documented chat-completion token accounting (role + separators
    /// + reply primer).
    ///
    /// Returns `(total_token_count, tier_used)`.  The tier is the same for
    /// every message because all messages are processed by the same dispatch
    /// path (BPE or heuristic).
    pub fn estimate_messages(messages: &[&str]) -> (usize, EstimationTier) {
        // Start with an empty accumulator.  We'll fold over every message,
        // adding its token count plus the per-message overhead.
        let mut total: usize = 0;
        // Default tier for the empty-slice case; will be overwritten on first
        // message otherwise.
        let mut tier = EstimationTier::Heuristic;

        for &msg in messages {
            let (count, t) = Self::estimate(msg);

            // Use saturating_add so we clamp to usize::MAX rather than
            // panicking or wrapping if someone passes a ludicrously long
            // conversation.  Token budgets work on order-of-millions, so
            // silent clamping is safe.
            total = total.saturating_add(count);
            total = total.saturating_add(CHAT_MESSAGE_OVERHEAD);

            // Every message uses the same tier, so just keep overwriting it
            // (avoids a special-case for the first iteration).
            tier = t;
        }

        (total, tier)
    }

    /// Returns `true` if `text` looks like source code, `false` if it looks
    /// like prose.
    ///
    /// Detection rule: if at least 15% of characters belong to the set
    /// `{ '{' '}' '(' ')' ';' '[' ']' '<' '>' '/' }` the text is treated as
    /// code-like.  This heuristic is intentionally simple and fast.
    ///
    /// Used internally by the heuristic estimator to pick the right divisor
    /// (code = / 3, prose = / 4).
    pub fn is_code_like(text: &str) -> bool {
        // An empty string is not code — avoids a division-by-zero edge case.
        if text.is_empty() {
            return false;
        }

        // Count total characters (Unicode-aware, not byte count).
        let total_chars = text.chars().count();

        // Count how many characters fall into the "code indicator" set.
        let code_char_count = text.chars().filter(|c| CODE_CHARS.contains(c)).count();

        // "code_char_count / total_chars >= 0.15"
        // Rearranged to avoid floating-point: multiply both sides by 100.
        code_char_count * 100 >= total_chars * 15
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Core heuristic: estimate tokens from byte length, biased by content type.
    ///
    /// - Code  → `byte_len / 3`  (code is token-dense: operators, brackets, etc.)
    /// - Prose → `byte_len / 4`  (prose is token-sparse: full English words)
    ///
    /// Always returns at least 1, even for a single-character input.
    #[allow(dead_code)]
    fn heuristic_estimate(text: &str) -> usize {
        let byte_len = text.len(); // len() is bytes, which is what we want here

        if Self::is_code_like(text) {
            // Code is more token-dense so we divide by a smaller number,
            // yielding a higher (more conservative) estimate.
            (byte_len / 3).max(1)
        } else {
            // Prose words are usually one token each, ~4 bytes on average.
            (byte_len / 4).max(1)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // is_code_like
    // -----------------------------------------------------------------------

    #[test]
    fn code_like_detects_rust_snippet() {
        // We avoid excessive block indentation in the raw string literal
        // because leading spaces inflate the denominator (total chars) and
        // dilute the ratio of code characters below our 15% threshold.
        let rust = r#"fn main() {
    let x: Vec<u32> = vec![1, 2, 3];
    for i in &x { println!("{}", i); }
}"#;
        assert!(
            TokenEstimator::is_code_like(rust),
            "Rust snippet should be classified as code-like"
        );
    }

    #[test]
    fn code_like_returns_false_for_english_prose() {
        let prose = "The quick brown fox jumps over the lazy dog. \
                     This is a simple English sentence with no code characters at all.";
        assert!(
            !TokenEstimator::is_code_like(prose),
            "Plain English prose should NOT be classified as code-like"
        );
    }

    #[test]
    fn code_like_returns_false_for_empty_string() {
        assert!(!TokenEstimator::is_code_like(""));
    }

    #[test]
    fn code_like_boundary_exactly_15_percent() {
        // Construct a string where exactly 15 out of 100 chars are code chars.
        // Should return true (>= 15%).
        let s = "a".repeat(85) + &"{".repeat(15);
        assert!(TokenEstimator::is_code_like(&s));
    }

    #[test]
    fn code_like_boundary_below_15_percent() {
        // 14 code chars out of 100 → < 15% → prose.
        let s = "a".repeat(86) + &"{".repeat(14);
        assert!(!TokenEstimator::is_code_like(&s));
    }

    // -----------------------------------------------------------------------
    // estimate — heuristic tier (always compiled)
    // -----------------------------------------------------------------------

    #[test]
    fn estimate_returns_at_least_one_for_single_char() {
        // Even a 1-byte string must return count >= 1.
        let (count, _tier) = TokenEstimator::estimate("a");
        assert!(count >= 1, "token count must be >= 1, got {count}");
    }

    #[test]
    fn estimate_short_prose_gives_sensible_count() {
        // "Hello, world!" is 13 bytes of prose → heuristic: 13/4 = 3 tokens.
        // BPE would give ~4. Either way it must be in a sane range.
        let (count, _tier) = TokenEstimator::estimate("Hello, world!");
        assert!(
            (1..=20).contains(&count),
            "expected 1-20 tokens for 'Hello, world!', got {count}"
        );
    }

    #[test]
    fn estimate_code_is_denser_than_same_length_prose() {
        // A code snippet of the same byte length should yield more tokens than
        // prose because code is token-dense (divisor 3 vs 4).
        // Only assert this for the heuristic path (BPE is accurate and may
        // differ).  Under heuristic, code tokens >= prose tokens.
        #[cfg(not(feature = "bpe"))]
        {
            let prose = "The quick brown fox jumps over the lazy dog.";
            let code = "fn foo(){let x=vec![1,2,3];for i in &x{println!();}}";
            let len = prose.len().min(code.len());
            let (prose_count, _) = TokenEstimator::estimate(&prose[..len]);
            let (code_count, _) = TokenEstimator::estimate(&code[..len]);
            assert!(
                code_count >= prose_count,
                "code ({code_count}) should estimate >= prose ({prose_count}) for same length"
            );
        }
    }

    #[test]
    fn estimate_empty_string_returns_one() {
        // Empty string has at least 1 token (model sees a blank turn).
        let (count, _tier) = TokenEstimator::estimate("");
        assert!(count >= 1);
    }

    // -----------------------------------------------------------------------
    // estimate_messages
    // -----------------------------------------------------------------------

    #[test]
    fn estimate_messages_adds_overhead_per_message() {
        // Single-message case: result must be >= individual estimate + overhead.
        let msg = "Hello, world!";
        let (single, _) = TokenEstimator::estimate(msg);
        let (total, _) = TokenEstimator::estimate_messages(&[msg]);
        assert_eq!(
            total,
            single + CHAT_MESSAGE_OVERHEAD,
            "single-message total should equal estimate + overhead"
        );
    }

    #[test]
    fn estimate_messages_empty_slice_returns_zero() {
        let (count, _tier) = TokenEstimator::estimate_messages(&[]);
        assert_eq!(count, 0);
    }

    #[test]
    fn estimate_messages_two_messages_sum_correctly() {
        let msgs = ["Hello", "World"];
        let (a, _) = TokenEstimator::estimate(msgs[0]);
        let (b, _) = TokenEstimator::estimate(msgs[1]);
        let expected = a + CHAT_MESSAGE_OVERHEAD + b + CHAT_MESSAGE_OVERHEAD;
        let (total, _) =
            TokenEstimator::estimate_messages(&msgs.iter().map(|s| *s).collect::<Vec<_>>());
        assert_eq!(total, expected);
    }

    // -----------------------------------------------------------------------
    // Tier tagging
    // -----------------------------------------------------------------------

    #[test]
    #[cfg(not(feature = "bpe"))]
    fn estimate_returns_heuristic_tier_without_bpe_feature() {
        let (_count, tier) = TokenEstimator::estimate("some text");
        assert_eq!(tier, EstimationTier::Heuristic);
    }

    #[test]
    #[cfg(feature = "bpe")]
    fn estimate_returns_bpe_tier_with_bpe_feature() {
        let (_count, tier) = TokenEstimator::estimate("some text");
        assert_eq!(tier, EstimationTier::Bpe);
    }
}
