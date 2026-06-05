// model.rs — Model configuration for operon-providers.
//
// `ModelConfig` bundles the three model-specific values every provider request needs:
//   - model_id: the exact string sent in the request body's "model" field
//   - context_window: total token budget of the model (input + output combined)
//   - max_tokens: maximum output tokens requested per turn
//
// Known model presets are provided as associated functions (not const items) because
// `String` is not const-constructible in stable Rust. Each preset is a `fn` that
// returns a freshly allocated `ModelConfig` — the cost is trivial (one small heap
// alloc) and only happens once at session startup.
//
// TODO: When operon-config is built, ModelConfig will be loadable from TOML so
// operators can define any model without code changes. These presets serve as
// documented, type-safe defaults.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// ModelConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for a specific LLM model.
///
/// Groups the three values the session runner needs for every API request:
/// the model identifier, the context window size, and the max output token budget.
///
/// # Example
///
/// Construct manually:
/// ```rust
/// use operon_providers::model::ModelConfig;
///
/// let model = ModelConfig {
///     model_id: "claude-3-5-sonnet-latest".to_string(),
///     context_window: 200_000,
///     max_tokens: 8_192,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// The exact model identifier string sent in the request body.
    ///
    /// Examples:
    /// - Anthropic: `"claude-sonnet-4-20250514"`, `"claude-opus-4-20250514"`
    /// - OpenAI:    `"gpt-4o"`, `"o3"`, `"o4-mini"`
    /// - Gemini:    `"gemini-2.5-pro"`, `"gemini-2.0-flash"`
    /// - Ollama:    `"llama3.2"`, `"qwen2.5-coder:32b"` (Ollama registry tag)
    pub model_id: String,

    /// Total token capacity of the model's context window (input + output combined).
    ///
    /// Used by `operon-context-token-tracker` to compute the compaction trigger
    /// threshold (e.g. "compact when 80% of the window is used").
    ///
    /// Common values: 200,000 (Claude), 128,000 (GPT-4o), 1,048,576 (Gemini 2.5 Pro).
    pub context_window: usize,

    /// Maximum output tokens to request per turn (`max_tokens` in request body).
    ///
    /// Must be ≤ the model's output token limit (separate from context_window).
    /// Typical values: 4,096–32,768. Higher values allow longer responses but
    /// increase cost and latency.
    pub max_tokens: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_serialization_roundtrip() {
        let original = ModelConfig {
            model_id: "claude-3-5-sonnet-latest".to_string(),
            context_window: 200_000,
            max_tokens: 8_192,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(original.model_id, restored.model_id);
        assert_eq!(original.context_window, restored.context_window);
        assert_eq!(original.max_tokens, restored.max_tokens);
    }
}
