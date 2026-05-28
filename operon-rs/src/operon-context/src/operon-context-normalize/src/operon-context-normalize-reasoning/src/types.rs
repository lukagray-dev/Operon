//! Canonical types for reasoning/thinking content.
//!
//! These are the stable, provider-agnostic representations used throughout
//! the Operon pipeline. All provider-specific wire formats are normalized into
//! [`ReasoningBlock`] on the way in, and denormalized back to provider format
//! on the way out.
//!
//! Both types derive `Debug`, `Clone`, `PartialEq`, `Serialize`, and
//! `Deserialize` so they can be stored, compared, and round-tripped through
//! JSON without extra boilerplate.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// ReasoningSignature
// ─────────────────────────────────────────────────────────────────────────────

/// An opaque provider signature that **must** be echoed back verbatim in
/// subsequent request turns.
///
/// Currently required by:
/// - **Anthropic** — the `"signature"` field on a `thinking` content block.
/// - **Gemini 3** — the `"thoughtSignature"` field on a thought part, required
///   when function calling is involved.
///
/// Omitting a required signature causes provider-side 4xx errors. Treat this
/// value as a black box — never inspect, parse, or modify the inner string.
///
/// # Example
/// ```
/// use operon_context_normalize_reasoning::ReasoningSignature;
///
/// let sig = ReasoningSignature("EqoBCkgIAR...".to_string());
/// // Echo it back exactly as-is in subsequent turns
/// assert_eq!(sig.0, "EqoBCkgIAR...");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningSignature(pub String);

// ─────────────────────────────────────────────────────────────────────────────
// ReasoningBlock
// ─────────────────────────────────────────────────────────────────────────────

/// A normalized reasoning/thinking block from any supported provider.
///
/// This is the canonical representation of a model's chain-of-thought trace,
/// regardless of which provider produced it. All provider wire formats are
/// converted to and from this type via [`normalize_reasoning`] and
/// [`denormalize_reasoning`].
///
/// # Fields
///
/// - `thinking` — the actual thinking text the model produced.
/// - `signature` — an opaque token that some providers require echoed back in
///   subsequent turns (Anthropic, Gemini 3). `None` for providers that do not
///   use signatures (OpenAI, DeepSeek, xAI, Ollama).
///
/// # Example
/// ```
/// use operon_context_normalize_reasoning::ReasoningBlock;
///
/// // Provider without signatures (OpenAI, DeepSeek, Ollama, xAI)
/// let block = ReasoningBlock::new("I need to think step by step.");
/// assert_eq!(block.thinking, "I need to think step by step.");
/// assert!(!block.has_signature());
///
/// // Provider with signatures (Anthropic, Gemini)
/// let block = ReasoningBlock::with_signature("My analysis...", "EqoBCkgIAR...");
/// assert!(block.has_signature());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningBlock {
    /// The chain-of-thought text the model produced during its reasoning pass.
    pub thinking: String,

    /// Opaque provider signature. Must be echoed verbatim in subsequent turns
    /// when present (Anthropic, Gemini 3). Absent for providers that do not
    /// issue signatures.
    pub signature: Option<ReasoningSignature>,
}

impl ReasoningBlock {
    /// Create a `ReasoningBlock` with thinking text and **no** signature.
    ///
    /// Use this constructor for providers that do not return a signature:
    /// OpenAI, DeepSeek, xAI, and Ollama.
    ///
    /// # Example
    /// ```
    /// use operon_context_normalize_reasoning::ReasoningBlock;
    ///
    /// let block = ReasoningBlock::new("Step 1: break the problem down.");
    /// assert_eq!(block.thinking, "Step 1: break the problem down.");
    /// assert!(block.signature.is_none());
    /// ```
    pub fn new(thinking: impl Into<String>) -> Self {
        Self {
            thinking: thinking.into(),
            // No signature — most providers omit this entirely
            signature: None,
        }
    }

    /// Create a `ReasoningBlock` with both thinking text and a signature.
    ///
    /// Use this constructor when a provider returns a signature alongside
    /// the thinking content — currently Anthropic and Gemini. The signature
    /// is an opaque string that must be preserved exactly and echoed back.
    ///
    /// # Example
    /// ```
    /// use operon_context_normalize_reasoning::ReasoningBlock;
    ///
    /// let block = ReasoningBlock::with_signature(
    ///     "I need to approach this carefully.",
    ///     "EqoBCkgIARISCPIB...",
    /// );
    /// assert!(block.has_signature());
    /// assert_eq!(block.signature.unwrap().0, "EqoBCkgIARISCPIB...");
    /// ```
    pub fn with_signature(thinking: impl Into<String>, sig: impl Into<String>) -> Self {
        Self {
            thinking: thinking.into(),
            // Wrap the raw signature string in the newtype
            signature: Some(ReasoningSignature(sig.into())),
        }
    }

    /// Returns `true` if this block carries a provider signature.
    ///
    /// When `true`, the caller **must** echo the `signature` back to the
    /// provider in subsequent request turns to avoid 4xx API errors.
    ///
    /// # Example
    /// ```
    /// use operon_context_normalize_reasoning::ReasoningBlock;
    ///
    /// let with_sig    = ReasoningBlock::with_signature("thinking text", "sig");
    /// let without_sig = ReasoningBlock::new("thinking text");
    ///
    /// assert!(with_sig.has_signature());
    /// assert!(!without_sig.has_signature());
    /// ```
    pub fn has_signature(&self) -> bool {
        self.signature.is_some()
    }
}
