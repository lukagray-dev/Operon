// discovery.rs — Model discovery via provider APIs.

use crate::Provider;
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Public Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredModel {
    pub model_id: String,
    pub context_window: usize,
    pub max_tokens: usize,
    #[serde(default)]
    pub description: String,
    /// Available reasoning effort / thinking levels for this model (e.g. "Low", "Medium", "High", "Max", "Disabled").
    /// If the provider does not provide reasoning capabilities for this model, this is empty.
    #[serde(default)]
    pub reasoning_levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub models: Vec<DiscoveredModel>,
    #[serde(skip)]
    pub provider: Option<Provider>,
}

use crate::reasoning::detect_model_reasoning_levels;

// ─────────────────────────────────────────────────────────────────────────────
// Response Types for Different Providers
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
    #[serde(default)]
    owned_by: String,
    context_window: Option<usize>,
    context_length: Option<usize>,
    max_tokens: Option<usize>,
    max_output_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Debug, Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: String,
    context_window: Option<usize>,
    context_length: Option<usize>,
    max_tokens: Option<usize>,
    max_output_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "inputTokenLimit")]
    input_token_limit: Option<usize>,
    #[serde(rename = "outputTokenLimit")]
    output_token_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    size: u64,
}

#[derive(Debug, Deserialize)]
struct OllamaShowResponse {
    model_info: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterModel {
    id: String,
    name: String,
    context_length: Option<usize>,
    #[serde(default)]
    description: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

pub async fn discover_models(
    provider: Provider,
    api_key: &str,
    base_url: Option<&str>,
) -> Result<DiscoveryResult, String> {
    let capabilities = provider.capabilities();
    let url = base_url.unwrap_or(capabilities.default_base_url);

    match provider {
        Provider::Anthropic => discover_anthropic(api_key, url).await,

        Provider::OpenAI
        | Provider::Groq
        | Provider::Mistral
        | Provider::XAI
        | Provider::DeepSeek
        | Provider::NvidiaNim => discover_openai_compatible(provider, api_key, url).await,

        Provider::Gemini => discover_gemini(api_key, url).await,

        Provider::Ollama => discover_ollama(url).await,

        Provider::Cohere => discover_cohere(api_key, url).await,

        Provider::OpenRouter => discover_openrouter(api_key, url).await,
    }
}

async fn discover_anthropic(api_key: &str, base_url: &str) -> Result<DiscoveryResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Operon/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API returned {}: {}", status, body));
    }

    let api_response: AnthropicModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models: Vec<DiscoveredModel> = api_response
        .data
        .into_iter()
        .map(|m| {
            // Hey friend! We follow a robust 3-tier context discovery pipeline:
            // 1. If the provider sends context window/length, use it directly.
            // 2. If missing, look up from our embedded model catalog dataset & family patterns.
            // 3. If completely unknown, fall back to a modern 128k default.
            let context_window = m
                .context_window
                .or(m.context_length)
                .unwrap_or_else(|| crate::catalog::lookup_context_window(&m.id));
            let max_tokens = m
                .max_tokens
                .or(m.max_output_tokens)
                .unwrap_or_else(|| crate::catalog::lookup_max_tokens(&m.id));
            let reasoning_levels = detect_model_reasoning_levels(Provider::Anthropic, &m.id, None);
            Ok(DiscoveredModel {
                model_id: m.id.clone(),
                context_window,
                max_tokens,
                description: if m.display_name.is_empty() {
                    m.id.clone()
                } else {
                    m.display_name
                },
                reasoning_levels,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(DiscoveryResult {
        models,
        provider: Some(Provider::Anthropic),
    })
}

async fn discover_openai_compatible(
    provider: Provider,
    api_key: &str,
    base_url: &str,
) -> Result<DiscoveryResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Operon/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API returned {}: {}", status, body));
    }

    let api_response: OpenAIModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models: Vec<DiscoveredModel> = api_response
        .data
        .into_iter()
        .map(|m| {
            // Hey friend! Many OpenAI-compatible providers (like NVIDIA NIM, DeepSeek, or
            // Groq) do not include context window metadata in their /v1/models JSON.
            // Our 3-tier pipeline seamlessly resolves context window from our embedded catalog!
            let context_window = m
                .context_window
                .or(m.context_length)
                .unwrap_or_else(|| crate::catalog::lookup_context_window(&m.id));
            let max_tokens = m
                .max_tokens
                .or(m.max_output_tokens)
                .unwrap_or_else(|| crate::catalog::lookup_max_tokens(&m.id));
            let reasoning_levels = detect_model_reasoning_levels(provider, &m.id, None);
            Ok(DiscoveredModel {
                model_id: m.id.clone(),
                context_window,
                max_tokens,
                description: m.owned_by,
                reasoning_levels,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(DiscoveryResult {
        models,
        provider: Some(provider),
    })
}

async fn discover_gemini(api_key: &str, base_url: &str) -> Result<DiscoveryResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Operon/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/models?key={}", base_url.trim_end_matches('/'), api_key);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API returned {}: {}", status, body));
    }

    let api_response: GeminiModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models = api_response
        .models
        .into_iter()
        .map(|m| {
            let model_id = m
                .name
                .strip_prefix("models/")
                .unwrap_or(&m.name)
                .to_string();
            let context_window = m
                .input_token_limit
                .unwrap_or_else(|| crate::catalog::lookup_context_window(&model_id));
            let max_tokens = m
                .output_token_limit
                .unwrap_or_else(|| crate::catalog::lookup_max_tokens(&model_id));
            let reasoning_levels = detect_model_reasoning_levels(Provider::Gemini, &model_id, None);
            DiscoveredModel {
                model_id,
                context_window,
                max_tokens,
                description: m.description,
                reasoning_levels,
            }
        })
        .collect();

    Ok(DiscoveryResult {
        models,
        provider: Some(Provider::Gemini),
    })
}

async fn discover_ollama(base_url: &str) -> Result<DiscoveryResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Operon/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API returned {}: {}", status, body));
    }

    let api_response: OllamaTagsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let show_url = format!("{}/api/show", base_url.trim_end_matches('/'));
    let mut models = Vec::new();

    for m in api_response.models {
        let show_payload = serde_json::json!({ "model": m.name });
        let show_resp = client.post(&show_url).json(&show_payload).send().await;

        let mut context_window = None;
        if let Ok(resp) = show_resp {
            if resp.status().is_success() {
                if let Ok(show_data) = resp.json::<OllamaShowResponse>().await {
                    context_window = show_data
                        .model_info
                        .iter()
                        .find(|(k, _)| k.ends_with(".context_length"))
                        .and_then(|(_, v)| v.as_u64())
                        .map(|v| v as usize);
                }
            }
        }

        let ctx = context_window.unwrap_or_else(|| crate::catalog::lookup_context_window(&m.name));
        let max_tokens = crate::catalog::lookup_max_tokens(&m.name);
        let reasoning_levels = detect_model_reasoning_levels(Provider::Ollama, &m.name, None);
        models.push(DiscoveredModel {
            model_id: m.name.clone(),
            context_window: ctx,
            max_tokens,
            description: format!("Size: {} GB", m.size / 1_000_000_000),
            reasoning_levels,
        });
    }

    Ok(DiscoveryResult {
        models,
        provider: Some(Provider::Ollama),
    })
}

async fn discover_cohere(api_key: &str, base_url: &str) -> Result<DiscoveryResult, String> {
    discover_openai_compatible(Provider::Cohere, api_key, base_url).await
}

async fn discover_openrouter(api_key: &str, base_url: &str) -> Result<DiscoveryResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Operon/1.0")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API returned {}: {}", status, body));
    }

    let api_response: OpenRouterModelsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let models = api_response
        .data
        .into_iter()
        .map(|m| {
            let context_window = m
                .context_length
                .unwrap_or_else(|| crate::catalog::lookup_context_window(&m.id));
            let max_tokens = crate::catalog::lookup_max_tokens(&m.id);
            let reasoning_levels = detect_model_reasoning_levels(Provider::OpenRouter, &m.id, None);
            DiscoveredModel {
                model_id: m.id,
                context_window,
                max_tokens,
                description: if m.description.is_empty() {
                    m.name
                } else {
                    m.description
                },
                reasoning_levels,
            }
        })
        .collect();

    Ok(DiscoveryResult {
        models,
        provider: Some(Provider::OpenRouter),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_reasoning_levels_from_direct_array() {
        let payload = serde_json::json!({
            "reasoning_levels": ["Low", "Medium", "High", "Max"]
        });
        let levels =
            detect_model_reasoning_levels(Provider::Anthropic, "claude-3-7-sonnet", Some(&payload));
        assert_eq!(levels, vec!["Low", "Medium", "High", "Max"]);
    }

    #[test]
    fn test_extract_reasoning_levels_from_nested_object() {
        let payload = serde_json::json!({
            "reasoning": {
                "levels": ["Low", "Mid", "High", "xHigh"]
            }
        });
        let levels =
            detect_model_reasoning_levels(Provider::OpenRouter, "some-model", Some(&payload));
        assert_eq!(levels, vec!["Low", "Mid", "High", "xHigh"]);
    }

    #[test]
    fn test_model_without_reasoning_metadata_returns_empty() {
        let payload = serde_json::json!({
            "id": "gpt-4o",
            "owned_by": "openai"
        });
        let levels = detect_model_reasoning_levels(Provider::OpenAI, "gpt-4o", Some(&payload));
        assert!(levels.is_empty());
    }

    #[test]
    fn test_none_payload_returns_empty() {
        let levels = detect_model_reasoning_levels(Provider::Anthropic, "claude-3-5-sonnet", None);
        assert!(levels.is_empty());
    }

    #[test]
    fn test_gemini_37_and_thinking_models() {
        let levels_37 = detect_model_reasoning_levels(Provider::Gemini, "gemini-3.7-flash", None);
        assert_eq!(levels_37, vec!["Low", "Medium", "High", "Disabled"]);

        let levels_25 = detect_model_reasoning_levels(Provider::Gemini, "gemini-2.5-pro", None);
        assert_eq!(levels_25, vec!["Low", "Medium", "High", "Disabled"]);

        let levels_tts =
            detect_model_reasoning_levels(Provider::Gemini, "gemini-2.5-flash-preview-tts", None);
        assert!(levels_tts.is_empty());
    }

    #[test]
    fn test_claude_37_sonnet() {
        let levels =
            detect_model_reasoning_levels(Provider::Anthropic, "claude-3-7-sonnet-20250219", None);
        assert_eq!(levels, vec!["Low", "Medium", "High", "Max", "Disabled"]);
    }
}
