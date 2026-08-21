// catalog.rs — Model specifications and context window database.
//
// Hey friend! Welcome to the model catalog module!
// In modern AI applications, LLMs have different context window limits (e.g. 1M for Gemini,
// 200k for Claude 3.7, 128k for Llama 3.3 and DeepSeek R1).
// While some providers (like OpenRouter) tell us the context window in their API responses,
// many others (like standard OpenAI endpoints, NVIDIA NIM, or custom local gateways) only
// return basic model IDs without any context window metadata.
//
// This module provides an embedded, lightning-fast database of model specifications:
// 1. First, we perform an exact or normalized lookup against our embedded catalog dataset.
// 2. If the exact model isn't in the dataset, we apply intelligent model-family heuristics
//    (e.g., any Gemini 2.x/1.5 gets 1M+, Claude 3.x gets 200k, Llama 3.x / DeepSeek gets 128k).
// 3. If it's a completely unknown new model, we default to a modern 128k context window
//    rather than an outdated 8k fallback!

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Specification metadata for a known language model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSpec {
    /// Total context window capacity in tokens (e.g. 128,000 or 1,048,576).
    pub context_window: usize,
    /// Maximum completion / output tokens the model can generate in a single turn.
    #[serde(default)]
    pub max_output_tokens: Option<usize>,
}

/// The embedded static JSON database loaded at compile time from `data/models.json`.
static EMBEDDED_MODELS_JSON: &str = include_str!("data/models.json");

/// Lazily parsed in-memory cache of model specifications for O(1) lookup.
static CATALOG_CACHE: OnceLock<HashMap<String, ModelSpec>> = OnceLock::new();

/// Returns the global in-memory catalog map, parsing `data/models.json` on first access.
fn get_catalog() -> &'static HashMap<String, ModelSpec> {
    CATALOG_CACHE.get_or_init(|| serde_json::from_str(EMBEDDED_MODELS_JSON).unwrap_or_default())
}

/// Normalizes a model identifier for resilient matching.
///
/// Strips provider prefixes (e.g., `meta/llama-3.3-70b` -> `llama-3.3-70b`),
/// converts to lowercase, and trims whitespace.
pub fn normalize_model_id(raw_id: &str) -> String {
    let trimmed = raw_id.trim().to_lowercase();

    // Hey friend! Many gateways (like NVIDIA NIM or OpenRouter) prefix model IDs with the
    // creator organization (e.g. "meta/llama-3.3-70b-instruct" or "deepseek-ai/deepseek-r1").
    // We strip these prefixes to match the canonical model name cleanly.
    let stripped = if let Some((_prefix, name)) = trimmed.split_once('/') {
        name
    } else {
        &trimmed
    };

    stripped.to_string()
}

/// Looks up the model specification from the embedded dataset or family heuristics.
///
/// Returns `Some(ModelSpec)` if found in the catalog or matched by a family pattern.
pub fn lookup_model_spec(model_id: &str) -> Option<ModelSpec> {
    let normalized = normalize_model_id(model_id);
    let catalog = get_catalog();

    // ── 1. Direct match on normalized ID ─────────────────────────────────────
    if let Some(spec) = catalog.get(&normalized) {
        return Some(spec.clone());
    }

    // ── 2. Strip standard revision / date suffixes ───────────────────────────
    // e.g. "claude-3-7-sonnet-20250219" -> "claude-3-7-sonnet"
    if let Some(base_name) = strip_date_suffix(&normalized) {
        if let Some(spec) = catalog.get(&base_name) {
            return Some(spec.clone());
        }
    }

    // ── 3. Model family heuristics ───────────────────────────────────────────
    detect_family_spec(&normalized)
}

/// Looks up the context window for a given model ID with a guaranteed modern default of 128k.
pub fn lookup_context_window(model_id: &str) -> usize {
    lookup_model_spec(model_id)
        .map(|spec| spec.context_window)
        .unwrap_or(128_000)
}

/// Looks up the maximum output tokens for a given model ID with a default of 8,192.
pub fn lookup_max_tokens(model_id: &str) -> usize {
    lookup_model_spec(model_id)
        .and_then(|spec| spec.max_output_tokens)
        .unwrap_or(8_192)
}

/// Strips date-like or revision suffixes (e.g. "-20250219", "-0125", "-2411").
fn strip_date_suffix(name: &str) -> Option<String> {
    if let Some((base, suffix)) = name.rsplit_once('-') {
        // If the suffix is purely numeric (like 20250219 or 0125 or 2411), strip it.
        if suffix.chars().all(|c| c.is_ascii_digit()) && suffix.len() >= 4 {
            return Some(base.to_string());
        }
    }
    None
}

/// Detects model specifications based on model family patterns when an exact ID match is missing.
fn detect_family_spec(name: &str) -> Option<ModelSpec> {
    // ── Gemini Family: 1M or 2M Context ──────────────────────────────────────
    if name.contains("gemini-2.0-pro") || name.contains("gemini-1.5-pro") {
        return Some(ModelSpec {
            context_window: 2_097_152,
            max_output_tokens: Some(8_192),
        });
    }
    if name.contains("gemini-2.5")
        || name.contains("gemini-2.0")
        || name.contains("gemini-1.5")
        || name.contains("gemini-3")
    {
        return Some(ModelSpec {
            context_window: 1_048_576,
            max_output_tokens: Some(65_536),
        });
    }

    // ── Claude Family: 200k Context ──────────────────────────────────────────
    if name.contains("claude-3-7") || name.contains("claude-3.7") {
        return Some(ModelSpec {
            context_window: 200_000,
            max_output_tokens: Some(128_000),
        });
    }
    if name.contains("claude-3-5")
        || name.contains("claude-3.5")
        || name.contains("claude-3")
        || name.contains("claude-4")
    {
        return Some(ModelSpec {
            context_window: 200_000,
            max_output_tokens: Some(8_192),
        });
    }

    // ── OpenAI o1 / o3 Reasoning Family: 200k Context ────────────────────────
    if name.starts_with("o1") || name.starts_with("o3") {
        return Some(ModelSpec {
            context_window: 200_000,
            max_output_tokens: Some(100_000),
        });
    }

    // ── Codestral: 256k Context ──────────────────────────────────────────────
    if name.contains("codestral") {
        return Some(ModelSpec {
            context_window: 256_000,
            max_output_tokens: Some(8_192),
        });
    }

    // ── Qwen 2.5 / Grok / DeepSeek / Llama 3.x Family: 128k - 131k Context ───
    if name.contains("qwen-2.5") || name.contains("qwen2.5") || name.contains("grok") {
        return Some(ModelSpec {
            context_window: 131_072,
            max_output_tokens: Some(8_192),
        });
    }

    if name.contains("llama-3.3")
        || name.contains("llama-3.2")
        || name.contains("llama-3.1")
        || name.contains("llama3.3")
        || name.contains("llama3.2")
        || name.contains("llama3.1")
        || name.contains("deepseek")
        || name.contains("mistral-large")
        || name.contains("mistral-small")
        || name.contains("gpt-4o")
        || name.contains("gpt-4-turbo")
        || name.contains("command-r")
    {
        return Some(ModelSpec {
            context_window: 128_000,
            max_output_tokens: Some(8_192),
        });
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_lookups_from_catalog() {
        assert_eq!(lookup_context_window("gpt-4o"), 128_000);
        assert_eq!(lookup_context_window("claude-3-7-sonnet"), 200_000);
        assert_eq!(lookup_context_window("gemini-2.5-pro"), 1_048_576);
        assert_eq!(lookup_context_window("deepseek-r1"), 128_000);
        assert_eq!(lookup_context_window("qwen-2.5-coder-32b-instruct"), 32_768);
        assert_eq!(lookup_context_window("codestral-latest"), 32_000);
    }

    #[test]
    fn test_prefixed_model_names_like_nvidia_nim() {
        assert_eq!(
            lookup_context_window("meta/llama-3.3-70b-instruct"),
            128_000
        );
        assert_eq!(lookup_context_window("deepseek-ai/deepseek-r1"), 128_000);
        assert_eq!(
            lookup_context_window("qwen/qwen-2.5-coder-32b-instruct"),
            32_768
        );
        assert_eq!(
            lookup_context_window("nvidia/llama-3.1-nemotron-70b-instruct"),
            128_000
        );
    }

    #[test]
    fn test_family_heuristics_for_unlisted_revisions() {
        // Unknown future sub-version of Gemini 2.5
        assert_eq!(
            lookup_context_window("gemini-2.5-flash-experimental-9999"),
            1_048_576
        );
        // Unknown future Claude 3.7 model
        assert_eq!(
            lookup_context_window("anthropic/claude-3-7-sonnet-custom"),
            200_000
        );
        // Unknown Llama 3.3 fine-tune
        assert_eq!(
            lookup_context_window("unsloth/llama-3.3-70b-instruct-bnb-4bit"),
            128_000
        );
    }

    #[test]
    fn test_fallback_defaults_to_128k() {
        assert_eq!(
            lookup_context_window("completely-unknown-custom-model-2026"),
            128_000
        );
        assert_eq!(
            lookup_max_tokens("completely-unknown-custom-model-2026"),
            8_192
        );
    }
}
