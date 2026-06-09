// model_commands.rs — Tauri IPC command handlers for model providers.

use operon_rs::{load as load_config, ApiCredentials, ModelConfig, Provider, ProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Shared State
// ─────────────────────────────────────────────────────────────────────────────

/// Application state shared across all command handlers.
#[derive(Default)]
pub struct AppState {
    /// Currently active provider configuration.
    /// Updated when user saves a provider setup.
    pub active_config: Option<ProviderConfig>,

    /// Cached model lists per provider, keyed by provider name.
    pub discovered_models: HashMap<String, Vec<ModelInfo>>,

    /// Active session control channels, keyed by session_id.
    pub active_sessions:
        HashMap<String, tokio::sync::mpsc::Sender<operon_rs::events::SessionCommand>>,
}

pub type SharedState = Arc<Mutex<AppState>>;

// ─────────────────────────────────────────────────────────────────────────────
// Data Transfer Objects (DTOs)
// ─────────────────────────────────────────────────────────────────────────────

/// Provider summary for the list view.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSummary {
    pub id: String,
    pub label: String,
    pub default_api_base: String,
    pub docs_url: String,
    pub requires_api_key: bool,
    pub is_active: bool,
    pub is_configured: bool,
    pub active_model: String,
}

/// Detailed provider setup for the configuration view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSetup {
    pub provider_id: String,
    pub label: String,
    pub default_api_base: String,
    pub docs_url: String,
    pub requires_api_key: bool,
    pub api_base: String,
    #[serde(default)]
    pub api_key: String,
    pub selected_model: String,
    #[serde(default)]
    pub fallback_models: Vec<String>,
    pub is_active: bool,
}

/// Model information returned from discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub model_id: String,
    pub context_window: usize,
    pub max_tokens: usize,
    #[serde(default)]
    pub description: String,
}

/// Request payload for discovering models.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverModelsRequest {
    pub provider_id: String,
    pub api_base: String,
    pub api_key: String,
}

/// Request payload for saving provider setup.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderRequest {
    pub provider_id: String,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

/// Response from model discovery.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverModelsResponse {
    pub models: Vec<ModelInfo>,
    pub active_model: String,
}

/// Response from save operation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderResponse {
    pub model: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

/// List all supported model providers.
#[tauri::command]
pub async fn get_model_providers(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<ProviderSummary>, String> {
    let state_guard = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;

    let active_provider_id = state_guard
        .active_config
        .as_ref()
        .map(|cfg| provider_to_id(&cfg.provider))
        .unwrap_or_default();

    let active_model = state_guard
        .active_config
        .as_ref()
        .map(|cfg| cfg.model.model_id.clone())
        .unwrap_or_default();

    drop(state_guard);

    // Try to load config, but don't fail if it's invalid (e.g., missing API key on first run)
    let config_result = load_config();
    let (configured_provider_id, has_api_key) = if let Ok(config) = config_result {
        let provider_id = provider_to_id(&config.provider.provider);
        let has_key = !config.provider.credentials.api_key.is_empty();
        (provider_id, has_key)
    } else {
        // No valid config yet (first run) - no provider is configured
        (String::new(), false)
    };

    // Use Provider::all() from backend to iterate over all supported providers
    let providers: Vec<ProviderSummary> = Provider::all()
        .iter()
        .map(|&provider| {
            let provider_id = provider_to_id(&provider);
            let capabilities = provider.capabilities();
            let is_active = active_provider_id == provider_id;
            let is_configured = configured_provider_id == provider_id;
            let requires_key = matches!(
                capabilities.auth_header,
                operon_rs::AuthHeader::Bearer
                    | operon_rs::AuthHeader::XApiKey
                    | operon_rs::AuthHeader::XGoogApiKey
            ) && provider != Provider::Ollama;

            ProviderSummary {
                id: provider_id.clone(),
                label: provider.display_name().to_string(),
                default_api_base: capabilities.default_base_url.to_string(),
                docs_url: get_provider_docs_url(&provider_id),
                requires_api_key: requires_key,
                is_active,
                is_configured: is_configured && (!requires_key || has_api_key),
                active_model: if is_active {
                    active_model.clone()
                } else {
                    String::new()
                },
            }
        })
        .collect();

    Ok(providers)
}

/// Get detailed setup for a specific provider.
#[tauri::command]
pub async fn get_model_provider_setup(
    provider_id: String,
    state: tauri::State<'_, SharedState>,
) -> Result<ProviderSetup, String> {
    // Try to load config, but don't fail if it's invalid (first run)
    let config_result = load_config();

    let state_guard = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;
    let fallback_models = state_guard
        .discovered_models
        .get(&provider_id)
        .map(|models| models.iter().map(|m| m.model_id.clone()).collect())
        .unwrap_or_else(Vec::new);
    drop(state_guard);

    let provider_enum =
        id_to_provider(&provider_id).ok_or_else(|| format!("Unknown provider: {}", provider_id))?;
    let provider_capabilities = provider_enum.capabilities();

    let (current_provider_id, api_base, api_key, selected_model) = if let Ok(config) = config_result
    {
        let current_id = provider_to_id(&config.provider.provider);
        let is_current = current_id == provider_id;

        let base = if is_current {
            config
                .provider
                .base_url_override
                .clone()
                .unwrap_or_default()
        } else {
            String::new()
        };

        let key = if is_current {
            config.provider.credentials.api_key.expose().to_string()
        } else {
            String::new()
        };

        let model = if is_current {
            config.provider.model.model_id.clone()
        } else {
            String::new()
        };

        (current_id, base, key, model)
    } else {
        // No valid config yet - all empty
        (String::new(), String::new(), String::new(), String::new())
    };

    let is_current = current_provider_id == provider_id;
    let requires_key = matches!(
        provider_capabilities.auth_header,
        operon_rs::AuthHeader::Bearer
            | operon_rs::AuthHeader::XApiKey
            | operon_rs::AuthHeader::XGoogApiKey
    ) && provider_enum != Provider::Ollama;

    let setup = ProviderSetup {
        provider_id: provider_id.clone(),
        label: provider_enum.display_name().to_string(),
        default_api_base: provider_capabilities.default_base_url.to_string(),
        docs_url: get_provider_docs_url(&provider_id),
        requires_api_key: requires_key,
        api_base,
        api_key,
        selected_model,
        fallback_models,
        is_active: is_current,
    };

    Ok(setup)
}

/// Discover available models for a provider using provided credentials.
#[tauri::command]
pub async fn discover_models(
    request: DiscoverModelsRequest,
    state: tauri::State<'_, SharedState>,
) -> Result<DiscoverModelsResponse, String> {
    let provider_enum = id_to_provider(&request.provider_id)
        .ok_or_else(|| format!("Unknown provider: {}", request.provider_id))?;

    // Use the backend's model discovery function
    let discovery_result = operon_rs::discover_models(
        provider_enum,
        &request.api_key,
        if request.api_base.is_empty() {
            None
        } else {
            Some(request.api_base.as_str())
        },
    )
    .await
    .map_err(|e| format!("Model discovery failed: {}", e))?;

    // Convert DiscoveredModel to ModelInfo
    let models: Vec<ModelInfo> = discovery_result
        .models
        .into_iter()
        .map(|dm| ModelInfo {
            model_id: dm.model_id,
            context_window: dm.context_window,
            max_tokens: dm.max_tokens,
            description: dm.description,
        })
        .collect();

    // Cache the discovered models
    {
        let mut state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard
            .discovered_models
            .insert(request.provider_id.clone(), models.clone());
    }

    let active_model = models
        .first()
        .map(|m| m.model_id.clone())
        .unwrap_or_default();

    Ok(DiscoverModelsResponse {
        models,
        active_model,
    })
}

/// Save provider configuration and activate it.
#[tauri::command]
pub async fn save_provider_setup(
    request: SaveProviderRequest,
    state: tauri::State<'_, SharedState>,
) -> Result<SaveProviderResponse, String> {
    // Validate inputs
    if request.model.trim().is_empty() {
        return Err("Model ID cannot be empty".to_string());
    }

    let provider_enum = id_to_provider(&request.provider_id)
        .ok_or_else(|| format!("Unknown provider: {}", request.provider_id))?;

    if requires_api_key(&request.provider_id) && request.api_key.trim().is_empty() {
        return Err("API key is required for this provider".to_string());
    }

    // Build the provider configuration
    let api_base_override = if !request.api_base.trim().is_empty() {
        Some(request.api_base.trim().to_string())
    } else {
        None
    };

    let credentials = if !request.api_key.trim().is_empty() {
        ApiCredentials::with_key(request.api_key.as_str())
    } else {
        ApiCredentials::unauthenticated()
    };

    // Try to find model info from discovered models
    let model_info = {
        let state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard
            .discovered_models
            .get(&request.provider_id)
            .and_then(|models| models.iter().find(|m| m.model_id == request.model))
            .cloned()
    };

    let resolved_model_info = if let Some(info) = model_info {
        info
    } else {
        // Run discovery on the fly to fetch metadata
        let discovery_result = operon_rs::discover_models(
            provider_enum,
            &request.api_key,
            if request.api_base.is_empty() {
                None
            } else {
                Some(request.api_base.as_str())
            },
        )
        .await
        .map_err(|e| format!("Failed to fetch model metadata on the fly: {}", e))?;

        discovery_result
            .models
            .into_iter()
            .find(|m| m.model_id == request.model)
            .map(|dm| ModelInfo {
                model_id: dm.model_id,
                context_window: dm.context_window,
                max_tokens: dm.max_tokens,
                description: dm.description,
            })
            .ok_or_else(|| {
                format!(
                    "Model '{}' was not found in the list of available models for provider '{}'",
                    request.model, request.provider_id
                )
            })?
    };

    let model_config = ModelConfig {
        model_id: resolved_model_info.model_id,
        context_window: resolved_model_info.context_window,
        max_tokens: resolved_model_info.max_tokens,
    };

    let provider_config = ProviderConfig {
        provider: provider_enum,
        credentials,
        model: model_config,
        base_url_override: api_base_override,
    };

    // Update the active configuration
    {
        let mut state_guard = state
            .lock()
            .map_err(|e| format!("Failed to lock state: {}", e))?;
        state_guard.active_config = Some(provider_config.clone());
    }

    // Persist configuration to disk
    operon_rs::save_provider(&provider_config)
        .map_err(|e| format!("Failed to save configuration: {}", e))?;

    Ok(SaveProviderResponse {
        model: request.model,
    })
}

/// Get the currently active provider configuration.
#[tauri::command]
pub async fn get_active_provider(
    state: tauri::State<'_, SharedState>,
) -> Result<Option<ProviderSetup>, String> {
    let state_guard = state
        .lock()
        .map_err(|e| format!("Failed to lock state: {}", e))?;

    if let Some(config) = &state_guard.active_config {
        let provider_id = provider_to_id(&config.provider);
        let provider_capabilities = config.provider.capabilities();
        let fallback_models = state_guard
            .discovered_models
            .get(&provider_id)
            .map(|models| models.iter().map(|m| m.model_id.clone()).collect())
            .unwrap_or_else(Vec::new);

        let requires_key = matches!(
            provider_capabilities.auth_header,
            operon_rs::AuthHeader::Bearer
                | operon_rs::AuthHeader::XApiKey
                | operon_rs::AuthHeader::XGoogApiKey
        ) && config.provider != Provider::Ollama;

        let setup = ProviderSetup {
            provider_id: provider_id.clone(),
            label: config.provider.display_name().to_string(),
            default_api_base: provider_capabilities.default_base_url.to_string(),
            docs_url: get_provider_docs_url(&provider_id),
            requires_api_key: requires_key,
            api_base: config.base_url_override.clone().unwrap_or_default(),
            api_key: config.credentials.api_key.expose().to_string(),
            selected_model: config.model.model_id.clone(),
            fallback_models,
            is_active: true,
        };

        Ok(Some(setup))
    } else {
        Ok(None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

fn provider_to_id(provider: &Provider) -> String {
    match provider {
        Provider::Anthropic => "anthropic".to_string(),
        Provider::OpenAI => "open_ai".to_string(),
        Provider::Gemini => "gemini".to_string(),
        Provider::Ollama => "ollama".to_string(),
        Provider::DeepSeek => "deep_seek".to_string(),
        Provider::OpenRouter => "open_router".to_string(),
        Provider::Groq => "groq".to_string(),
        Provider::Mistral => "mistral".to_string(),
        Provider::XAI => "xai".to_string(),
        Provider::NvidiaNim => "nvidia_nim".to_string(),
        Provider::Cohere => "cohere".to_string(),
    }
}

fn id_to_provider(id: &str) -> Option<Provider> {
    match id {
        "anthropic" => Some(Provider::Anthropic),
        "open_ai" => Some(Provider::OpenAI),
        "gemini" => Some(Provider::Gemini),
        "ollama" => Some(Provider::Ollama),
        "deep_seek" => Some(Provider::DeepSeek),
        "open_router" => Some(Provider::OpenRouter),
        "groq" => Some(Provider::Groq),
        "mistral" => Some(Provider::Mistral),
        "xai" => Some(Provider::XAI),
        "nvidia_nim" => Some(Provider::NvidiaNim),
        "cohere" => Some(Provider::Cohere),
        _ => None,
    }
}

fn get_provider_docs_url(id: &str) -> String {
    match id {
        "anthropic" => "https://docs.anthropic.com",
        "open_ai" => "https://platform.openai.com/docs",
        "gemini" => "https://ai.google.dev/docs",
        "ollama" => "https://ollama.ai/docs",
        "deep_seek" => "https://platform.deepseek.com/docs",
        "open_router" => "https://openrouter.ai/docs",
        "groq" => "https://console.groq.com/docs",
        "mistral" => "https://docs.mistral.ai",
        "xai" => "https://docs.x.ai",
        "nvidia_nim" => "https://docs.nvidia.com/nim",
        "cohere" => "https://docs.cohere.com",
        _ => "",
    }
    .to_string()
}

fn requires_api_key(id: &str) -> bool {
    if let Some(provider) = id_to_provider(id) {
        let capabilities = provider.capabilities();
        matches!(
            capabilities.auth_header,
            operon_rs::AuthHeader::Bearer
                | operon_rs::AuthHeader::XApiKey
                | operon_rs::AuthHeader::XGoogApiKey
        ) && provider != Provider::Ollama
    } else {
        true // Conservative default
    }
}
