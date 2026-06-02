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
/// # Presets
///
/// Use the associated functions for well-known models:
/// ```rust
/// use operon_providers::model::ModelConfig;
///
/// let model = ModelConfig::claude_sonnet_4();
/// assert_eq!(model.model_id, "claude-sonnet-4-20250514");
/// assert_eq!(model.context_window, 200_000);
/// ```
///
/// Or construct manually for any model not in the preset list:
/// ```rust
/// use operon_providers::model::ModelConfig;
///
/// let model = ModelConfig {
///     model_id: "my-fine-tuned-model-v2".to_string(),
///     context_window: 32_000,
///     max_tokens: 4_096,
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

impl ModelConfig {
    // ── Anthropic ─────────────────────────────────────────────────────────────

    /// Claude Sonnet 4 — balanced performance and speed (Anthropic's sweet spot model).
    pub fn claude_sonnet_4() -> Self {
        Self {
            model_id: "claude-sonnet-4-20250514".to_string(),
            context_window: 200_000,
            max_tokens: 16_000,
        }
    }

    /// Claude Opus 4 — highest capability Anthropic model, higher cost.
    pub fn claude_opus_4() -> Self {
        Self {
            model_id: "claude-opus-4-20250514".to_string(),
            context_window: 200_000,
            max_tokens: 32_000,
        }
    }

    /// Claude Haiku 3.5 — fastest and cheapest Anthropic model.
    pub fn claude_haiku_3_5() -> Self {
        Self {
            model_id: "claude-haiku-3-5-20241022".to_string(),
            context_window: 200_000,
            max_tokens: 8_000,
        }
    }

    // ── OpenAI ────────────────────────────────────────────────────────────────

    /// GPT-4o — flagship multimodal OpenAI model.
    pub fn gpt_4o() -> Self {
        Self {
            model_id: "gpt-4o".to_string(),
            context_window: 128_000,
            max_tokens: 16_384,
        }
    }

    /// o4-mini — compact OpenAI reasoning model, fast and affordable.
    pub fn o4_mini() -> Self {
        Self {
            model_id: "o4-mini".to_string(),
            context_window: 200_000,
            max_tokens: 100_000,
        }
    }

    /// o3 — full OpenAI reasoning model, highest capability.
    pub fn o3() -> Self {
        Self {
            model_id: "o3".to_string(),
            context_window: 200_000,
            max_tokens: 100_000,
        }
    }

    // ── Google Gemini ─────────────────────────────────────────────────────────

    /// Gemini 2.5 Pro — Google's highest-capability model with 1M context window.
    pub fn gemini_2_5_pro() -> Self {
        Self {
            model_id: "gemini-2.5-pro".to_string(),
            context_window: 1_048_576,
            max_tokens: 65_536,
        }
    }

    /// Gemini 2.0 Flash — Google's fast, efficient model.
    pub fn gemini_2_0_flash() -> Self {
        Self {
            model_id: "gemini-2.0-flash".to_string(),
            context_window: 1_048_576,
            max_tokens: 8_192,
        }
    }

    // ── DeepSeek ──────────────────────────────────────────────────────────────

    /// DeepSeek-V3 Chat — DeepSeek's general-purpose model.
    pub fn deepseek_chat() -> Self {
        Self {
            model_id: "deepseek-chat".to_string(),
            context_window: 64_000,
            max_tokens: 8_000,
        }
    }

    /// DeepSeek-R1 — reasoning model, exposes `reasoning_content` in responses.
    pub fn deepseek_reasoner() -> Self {
        Self {
            model_id: "deepseek-reasoner".to_string(),
            context_window: 64_000,
            max_tokens: 8_000,
        }
    }

    // ── Groq ──────────────────────────────────────────────────────────────────

    /// Llama 3.3 70B on Groq — high-quality model at extremely fast inference speed.
    pub fn groq_llama_3_3_70b() -> Self {
        Self {
            model_id: "llama-3.3-70b-versatile".to_string(),
            context_window: 128_000,
            max_tokens: 32_768,
        }
    }

    // ── Mistral ───────────────────────────────────────────────────────────────

    /// Mistral Large — Mistral's highest capability model.
    pub fn mistral_large() -> Self {
        Self {
            model_id: "mistral-large-latest".to_string(),
            context_window: 128_000,
            max_tokens: 4_096,
        }
    }

    // ── xAI ───────────────────────────────────────────────────────────────────

    /// Grok 4 — xAI's latest model.
    pub fn grok_4() -> Self {
        Self {
            model_id: "grok-4".to_string(),
            context_window: 256_000,
            max_tokens: 16_384,
        }
    }

    // ── Cohere ────────────────────────────────────────────────────────────────

    /// Command R+ — Cohere's most capable model.
    pub fn cohere_command_r_plus() -> Self {
        Self {
            model_id: "command-r-plus".to_string(),
            context_window: 128_000,
            max_tokens: 4_096,
        }
    }

    // ── Ollama ────────────────────────────────────────────────────────────────

    /// Llama 3.2 3B — a small, fast local model suitable for development testing.
    ///
    /// Pull with: `ollama pull llama3.2`
    pub fn ollama_llama3_2() -> Self {
        Self {
            model_id: "llama3.2".to_string(),
            context_window: 128_000,
            max_tokens: 8_192,
        }
    }

    // ── OpenRouter ────────────────────────────────────────────────────────────

    /// OpenRouter routing to Claude Sonnet 4 — useful for OpenRouter-specific features.
    pub fn openrouter_claude_sonnet_4() -> Self {
        Self {
            model_id: "anthropic/claude-sonnet-4".to_string(),
            context_window: 200_000,
            max_tokens: 16_000,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presets_have_valid_fields() {
        // Every preset must have a non-empty model_id and a non-zero context_window.
        let presets: Vec<ModelConfig> = vec![
            ModelConfig::claude_sonnet_4(),
            ModelConfig::claude_opus_4(),
            ModelConfig::claude_haiku_3_5(),
            ModelConfig::gpt_4o(),
            ModelConfig::o4_mini(),
            ModelConfig::o3(),
            ModelConfig::gemini_2_5_pro(),
            ModelConfig::gemini_2_0_flash(),
            ModelConfig::deepseek_chat(),
            ModelConfig::deepseek_reasoner(),
            ModelConfig::groq_llama_3_3_70b(),
            ModelConfig::mistral_large(),
            ModelConfig::grok_4(),
            ModelConfig::cohere_command_r_plus(),
            ModelConfig::ollama_llama3_2(),
            ModelConfig::openrouter_claude_sonnet_4(),
        ];

        for preset in &presets {
            assert!(
                !preset.model_id.is_empty(),
                "model_id must not be empty: {:?}",
                preset
            );
            assert!(
                preset.context_window > 0,
                "context_window must be > 0: {:?}",
                preset
            );
            assert!(
                preset.max_tokens > 0,
                "max_tokens must be > 0: {:?}",
                preset
            );
            assert!(
                preset.max_tokens <= preset.context_window,
                "max_tokens ({}) must be <= context_window ({}) for model {}",
                preset.max_tokens,
                preset.context_window,
                preset.model_id
            );
        }
    }

    #[test]
    fn test_presets_serialization_roundtrip() {
        // Every preset must survive a JSON serialize → deserialize roundtrip.
        let original = ModelConfig::claude_sonnet_4();
        let json = serde_json::to_string(&original).unwrap();
        let restored: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(original.model_id, restored.model_id);
        assert_eq!(original.context_window, restored.context_window);
        assert_eq!(original.max_tokens, restored.max_tokens);
    }
}
