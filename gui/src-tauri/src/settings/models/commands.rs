//! Models & Providers Settings Backend Tauri Commands.
//
// 1:1 match with Slint settings/main-content/models.rs:
// - Dynamic provider list loading from operon_rs::providers::Provider::all().
// - Provider credentials & base URL loading from ~/.operon/config.toml.
// - Real-time model auto-discovery using operon_rs::discover_models.
// - Full provider activation persistence via operon_rs::save_provider.

use super::types::{ProviderSetupDetailsDto, ProviderSummaryDto, SaveProviderRequestDto};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Thread-safe cache to hold discovered model IDs per provider.
fn discovered_models_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Helper to map Provider enum variant to lowercase ID string.
pub fn provider_to_id(provider: &operon_rs::providers::Provider) -> String {
    match provider {
        operon_rs::providers::Provider::Anthropic => "anthropic".to_string(),
        operon_rs::providers::Provider::OpenAI => "open_ai".to_string(),
        operon_rs::providers::Provider::Gemini => "gemini".to_string(),
        operon_rs::providers::Provider::Ollama => "ollama".to_string(),
        operon_rs::providers::Provider::DeepSeek => "deep_seek".to_string(),
        operon_rs::providers::Provider::OpenRouter => "open_router".to_string(),
        operon_rs::providers::Provider::Groq => "groq".to_string(),
        operon_rs::providers::Provider::Mistral => "mistral".to_string(),
        operon_rs::providers::Provider::XAI => "xai".to_string(),
        operon_rs::providers::Provider::NvidiaNim => "nvidia_nim".to_string(),
        operon_rs::providers::Provider::Cohere => "cohere".to_string(),
    }
}

/// Helper to map string ID back to Provider enum variant.
pub fn id_to_provider(id: &str) -> Option<operon_rs::providers::Provider> {
    match id {
        "anthropic" => Some(operon_rs::providers::Provider::Anthropic),
        "open_ai" => Some(operon_rs::providers::Provider::OpenAI),
        "gemini" => Some(operon_rs::providers::Provider::Gemini),
        "ollama" => Some(operon_rs::providers::Provider::Ollama),
        "deep_seek" => Some(operon_rs::providers::Provider::DeepSeek),
        "open_router" => Some(operon_rs::providers::Provider::OpenRouter),
        "groq" => Some(operon_rs::providers::Provider::Groq),
        "mistral" => Some(operon_rs::providers::Provider::Mistral),
        "xai" => Some(operon_rs::providers::Provider::XAI),
        "nvidia_nim" => Some(operon_rs::providers::Provider::NvidiaNim),
        "cohere" => Some(operon_rs::providers::Provider::Cohere),
        _ => None,
    }
}

/// Determines if a provider requires an API key.
fn requires_api_key(id: &str) -> bool {
    if let Some(provider) = id_to_provider(id) {
        let capabilities = provider.capabilities();
        matches!(
            capabilities.auth_header,
            operon_rs::providers::AuthHeader::Bearer
                | operon_rs::providers::AuthHeader::XApiKey
                | operon_rs::providers::AuthHeader::XGoogApiKey
        ) && provider != operon_rs::providers::Provider::Ollama
    } else {
        true
    }
}

/// Fetches the list of all supported model providers and their configured/active status.
#[tauri::command]
pub async fn get_providers_list() -> Result<Vec<ProviderSummaryDto>, String> {
    let app_config = operon_rs::load().ok();

    let active_provider_id = app_config
        .as_ref()
        .map(|c| provider_to_id(&c.provider.provider));
    let active_model = app_config
        .as_ref()
        .map(|c| c.provider.model.model_id.clone())
        .unwrap_or_default();

    let list = operon_rs::providers::Provider::all()
        .iter()
        .map(|&provider| {
            let provider_id = provider_to_id(&provider);
            let is_active = active_provider_id.as_deref() == Some(provider_id.as_str());

            let is_configured = if let Some(ref config) = app_config {
                provider_to_id(&config.provider.provider) == provider_id
                    && !config.provider.credentials.api_key.is_empty()
            } else {
                false
            };

            let status = if is_active || is_configured {
                "Configured".to_string()
            } else if requires_api_key(&provider_id) {
                "API key required".to_string()
            } else {
                "Not configured".to_string()
            };

            ProviderSummaryDto {
                id: provider_id,
                label: provider.display_name().to_string(),
                status,
                active_model: if is_active {
                    active_model.clone()
                } else {
                    "".to_string()
                },
                is_active,
            }
        })
        .collect();

    Ok(list)
}

/// Retrieves setup details and cached models for a specific provider.
#[tauri::command]
pub async fn get_provider_setup_details(
    provider_id: String,
) -> Result<ProviderSetupDetailsDto, String> {
    let provider_enum = id_to_provider(&provider_id)
        .ok_or_else(|| format!("Unknown provider id: {}", provider_id))?;

    let app_config = operon_rs::load().ok();
    let is_matching_active = app_config
        .as_ref()
        .is_some_and(|c| provider_to_id(&c.provider.provider) == provider_id);

    let mut saved_base = String::new();
    let mut saved_key = String::new();
    let mut saved_model = String::new();

    if is_matching_active {
        if let Some(ref config) = app_config {
            saved_base = config
                .provider
                .base_url_override
                .clone()
                .unwrap_or_default();
            saved_key = config.provider.credentials.api_key.expose().to_string();
            saved_model = config.provider.model.model_id.clone();
        }
    } else {
        saved_base = provider_enum.capabilities().default_base_url.to_string();
    }

    let cached_models = {
        let cache = discovered_models_cache().lock().unwrap();
        cache.get(&provider_id).cloned().unwrap_or_default()
    };

    Ok(ProviderSetupDetailsDto {
        provider_id: provider_id.clone(),
        provider_label: provider_enum.display_name().to_string(),
        api_base_url: saved_base,
        api_key: saved_key,
        active_model: saved_model,
        discovered_models: cached_models,
    })
}

/// Dynamically queries the provider's models endpoint and caches discovered models.
#[tauri::command]
pub async fn discover_provider_models(
    provider_id: String,
    api_base: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let provider_enum = id_to_provider(&provider_id)
        .ok_or_else(|| format!("Unknown provider id: {}", provider_id))?;

    let key_str = api_key.trim();
    let base_override = if api_base.trim().is_empty() {
        None
    } else {
        Some(api_base.trim())
    };

    match operon_rs::discover_models(provider_enum, key_str, base_override).await {
        Ok(result) => {
            let model_ids: Vec<String> = result.models.into_iter().map(|m| m.model_id).collect();
            let mut cache = discovered_models_cache().lock().unwrap();
            cache.insert(provider_id, model_ids.clone());
            Ok(model_ids)
        }
        Err(err) => Err(format!("Model discovery failed: {:#}", err)),
    }
}

/// Persists the selected provider credentials, base URL, and active model to config.toml.
#[tauri::command]
pub async fn save_provider_config(request: SaveProviderRequestDto) -> Result<(), String> {
    let provider_enum = id_to_provider(&request.provider_id)
        .ok_or_else(|| format!("Unknown provider id: {}", request.provider_id))?;

    let credentials = if !request.api_key.trim().is_empty() {
        operon_rs::ApiCredentials::with_key(request.api_key.trim())
    } else {
        operon_rs::ApiCredentials::unauthenticated()
    };

    let model_config = operon_rs::ModelConfig {
        model_id: request.selected_model,
        context_window: 128_000,
        max_tokens: 4_096,
        reasoning_effort: None,
    };

    let provider_config = operon_rs::ProviderConfig {
        provider: provider_enum,
        credentials,
        model: model_config,
        base_url_override: if request.api_base.trim().is_empty() {
            None
        } else {
            Some(request.api_base.trim().to_string())
        },
    };

    operon_rs::save_provider(&provider_config).map_err(|e| e.to_string())
}
