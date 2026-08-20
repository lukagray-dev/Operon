//! # Reasoning & Extended Thinking Capabilities
//!
//! Provider-specific capability detection and dynamic schema extraction for reasoning models.
//!
//! ## Architecture
//!
//! 1. **Dynamic Provider Payload Check**: If the provider or gateway API returns an explicit
//!    reasoning schema (e.g. `reasoning_levels`, `supported_reasoning_levels`, `thinking_levels`, etc.),
//!    we extract and honor those exact levels dynamically.
//! 2. **Provider Submodule Dispatch**: If the provider's listing API does not return reasoning schemas
//!    (such as Anthropic, Google Gemini, OpenAI, DeepSeek, Ollama), we delegate to the dedicated
//!    provider module (e.g., [`anthropic`], [`gemini`], [`openai`], [`deepseek`]).

pub mod anthropic;
pub mod cohere;
pub mod deepseek;
pub mod gemini;
pub mod groq;
pub mod mistral;
pub mod nvidia_nim;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod xai;

use crate::Provider;

/// Extracts dynamic reasoning levels from raw API JSON payload if present.
fn extract_dynamic_levels(val: &serde_json::Value) -> Option<Vec<String>> {
    // 1. Check direct array keys
    let candidate_keys = [
        "reasoning_levels",
        "supported_reasoning_levels",
        "reasoning_efforts",
        "supported_reasoning_efforts",
        "thinking_levels",
        "reasoning_effort_levels",
        "reasoning_options",
        "effort_levels",
    ];

    for key in candidate_keys {
        if let Some(arr) = val.get(key).and_then(|v| v.as_array()) {
            let levels: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !levels.is_empty() {
                return Some(levels);
            }
        }
    }

    // 2. Check nested objects (e.g., {"reasoning": {"levels": [...]}} or {"thinking": {"levels": [...]}})
    let nested_keys = ["reasoning", "thinking", "capabilities", "parameters"];
    for parent_key in nested_keys {
        if let Some(parent_obj) = val.get(parent_key).and_then(|v| v.as_object()) {
            for sub_key in ["levels", "effort_levels", "supported_levels", "options", "values"] {
                if let Some(arr) = parent_obj.get(sub_key).and_then(|v| v.as_array()) {
                    let levels: Vec<String> = arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    if !levels.is_empty() {
                        return Some(levels);
                    }
                }
            }
        }
    }

    None
}

/// Detects the supported reasoning levels for a given model from its provider.
///
/// Hey friend! If a model supports reasoning effort (like Claude 3.7 with Low/Med/High/Max
/// or Gemini 3.7 with Low/Med/High), this function returns the list of available levels.
/// If a model does NOT support reasoning (like standard GPT-4o or Claude 3.5), it returns
/// an empty `Vec` so the UI knows not to display a reasoning level selector.
pub fn detect_model_reasoning_levels(
    provider: Provider,
    model_id: &str,
    raw_info: Option<&serde_json::Value>,
) -> Vec<String> {
    // Step 1: Check dynamic JSON payload from provider API
    if let Some(val) = raw_info {
        if let Some(levels) = extract_dynamic_levels(val) {
            return levels;
        }
    }

    // Step 2: Delegate to provider-specific capability submodule
    match provider {
        Provider::Anthropic => anthropic::detect_anthropic_reasoning(model_id),
        Provider::Gemini => gemini::detect_gemini_reasoning(model_id),
        Provider::OpenAI => openai::detect_openai_reasoning(model_id),
        Provider::DeepSeek => deepseek::detect_deepseek_reasoning(model_id),
        Provider::Ollama => ollama::detect_ollama_reasoning(model_id),
        Provider::XAI => xai::detect_xai_reasoning(model_id),
        Provider::OpenRouter => openrouter::detect_openrouter_reasoning(model_id),
        Provider::Groq => groq::detect_groq_reasoning(model_id),
        Provider::NvidiaNim => nvidia_nim::detect_nvidia_nim_reasoning(model_id),
        Provider::Mistral => mistral::detect_mistral_reasoning(model_id),
        Provider::Cohere => cohere::detect_cohere_reasoning(model_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_json_payload_precedence() {
        let payload = serde_json::json!({
            "reasoning_levels": ["Minimal", "Standard", "Intense", "Disabled"]
        });
        let levels = detect_model_reasoning_levels(
            Provider::OpenRouter,
            "custom-unknown-model",
            Some(&payload),
        );
        assert_eq!(
            levels,
            vec!["Minimal", "Standard", "Intense", "Disabled"]
        );
    }

    #[test]
    fn test_gemini_and_claude_module_dispatch() {
        assert_eq!(
            detect_model_reasoning_levels(Provider::Gemini, "gemini-3.7-flash", None),
            vec!["Low", "Medium", "High", "Disabled"]
        );
        assert_eq!(
            detect_model_reasoning_levels(Provider::Anthropic, "claude-3-7-sonnet-20250219", None),
            vec!["Low", "Medium", "High", "Max", "Disabled"]
        );
    }
}
