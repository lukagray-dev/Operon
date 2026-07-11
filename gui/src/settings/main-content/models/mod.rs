//! Controller for the Models configuration settings page.
//!
//! This module wires the callbacks of the `ModelsSettings` UI component:
//! - Loads the list of providers from the `operon-rs` backend.
//! - Handles provider selection by loading current credentials/endpoints from the configuration.
//! - Executes dynamic model discovery using the `operon-rs` provider discovery APIs.
//! - Persists updated provider credentials and selected active models to `config.toml`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use slint::{ComponentHandle, ModelRc, VecModel, SharedString};

use crate::state::AppState;
use crate::ProviderSummary; // Slint-generated struct

/// Thread-safe cache to hold discovered model IDs per provider.
fn discovered_models_cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Helper function to map a Provider enum variant to its lowercase ID string.
fn provider_to_id(provider: &operon_rs::providers::Provider) -> String {
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

/// Helper function to map a string ID back to the canonical Provider enum variant.
fn id_to_provider(id: &str) -> Option<operon_rs::providers::Provider> {
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

/// Determines if a provider requires an API key for authentication.
/// Ollama is hosted locally and does not require an API key.
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

/// Fetches the list of all supported model providers, checking if they are
/// configured or currently active, and maps them to Slint DTOs.
pub fn get_providers_list() -> Vec<ProviderSummary> {
    // 1. Attempt to load the active configuration from ~/.operon/config.toml
    let app_config = operon_rs::load().ok();
    
    let active_provider_id = app_config.as_ref().map(|c| provider_to_id(&c.provider.provider));
    let active_model = app_config.as_ref().map(|c| c.provider.model.model_id.clone()).unwrap_or_default();

    // 2. Iterate over all providers supported by the backend
    operon_rs::providers::Provider::all()
        .iter()
        .map(|&provider| {
            let provider_id = provider_to_id(&provider);
            let is_active = active_provider_id.as_ref().map_or(false, |id| id == &provider_id);
            
            // Check if this provider has a configured API key in the config
            let is_configured = if let Some(ref config) = app_config {
                provider_to_id(&config.provider.provider) == provider_id 
                    && !config.provider.credentials.api_key.is_empty()
            } else {
                false
            };

            // Map status labels to show in the list view cards
            let status = if is_active || is_configured {
                "Configured".to_string()
            } else if requires_api_key(&provider_id) {
                "API key required".to_string()
            } else {
                "Not configured".to_string()
            };

            ProviderSummary {
                id: provider_id.into(),
                label: provider.display_name().into(),
                status: status.into(),
                active_model: if is_active { active_model.clone().into() } else { "".into() },
                is_active,
            }
        })
        .collect()
}

/// Triggers asynchronous model discovery for a provider, caches the results, and updates the UI.
fn trigger_model_discovery(
    window: &crate::SettingsWindow,
    provider_id: &str,
    api_base: &str,
    api_key: &str,
) {
    let provider_enum = match id_to_provider(provider_id) {
        Some(p) => p,
        None => {
            window.set_provider_models(ModelRc::from(Rc::new(VecModel::default())));
            return;
        }
    };

    let weak_window = window.as_weak();
    let provider_id_str = provider_id.to_string();
    let api_key_str = api_key.to_string();
    let api_base_str = api_base.to_string();

    tokio::spawn(async move {
        let base_opt = if api_base_str.trim().is_empty() { None } else { Some(api_base_str.as_str()) };
        match operon_rs::discover_models(provider_enum, &api_key_str, base_opt).await {
            Ok(result) => {
                println!("[operon-gui][settings] Model auto-discovery succeeded: found {} models", result.models.len());
                let model_ids: Vec<String> = result.models.into_iter().map(|m| m.model_id).collect();

                // Cache the list
                {
                    let mut cache = discovered_models_cache().lock().unwrap();
                    cache.insert(provider_id_str.clone(), model_ids.clone());
                }

                // Update the UI thread-safely
                let slint_models: Vec<SharedString> = model_ids.into_iter().map(SharedString::from).collect();
                let active_model = slint_models.first().cloned().unwrap_or_default();
                
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = weak_window.upgrade() {
                        // Only apply the updates if the active configured provider in the view has not changed
                        if win.get_selected_provider_id() == provider_id_str {
                            win.set_provider_models(ModelRc::from(Rc::new(VecModel::from(slint_models))));
                            if win.get_active_model().is_empty() {
                                win.set_active_model(active_model);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                eprintln!("[operon-gui][settings] Model auto-discovery failed: {}", e);
                // Clear the UI model list to reflect that discovery failed
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = weak_window.upgrade() {
                        if win.get_selected_provider_id() == provider_id_str {
                            win.set_provider_models(ModelRc::from(Rc::new(VecModel::default())));
                        }
                    }
                });
            }
        }
    });
}

/// Registers the callback handlers on the Settings window for Models category settings.
pub fn wire_models_settings(
    window: &crate::SettingsWindow,
    _state: Rc<RefCell<AppState>>,
) {
    let weak_window = window.as_weak();

    // Populate initial providers list in the UI
    let providers = get_providers_list();
    window.set_providers(ModelRc::from(Rc::new(VecModel::from(providers))));

    // Handler 1: Triggered when the user clicks a provider card to configure it
    window.on_provider_selected({
        let weak_window = weak_window.clone();
        move |provider_id, provider_label| {
            println!("[operon-gui][settings] Selected provider: {} ({})", provider_label, provider_id);
            if let Some(win) = weak_window.upgrade() {
                win.set_selected_provider_id(provider_id.clone());
                win.set_selected_provider_label(provider_label);

                // Load active config to see if we have credentials saved for this provider
                let app_config = operon_rs::load().ok();
                let is_matching_active = app_config.as_ref()
                    .map_or(false, |c| provider_to_id(&c.provider.provider) == provider_id.as_str());

                let mut saved_base = String::new();
                let mut saved_key = String::new();
                let mut saved_model = String::new();

                if is_matching_active {
                    if let Some(ref config) = app_config {
                        saved_base = config.provider.base_url_override.clone().unwrap_or_default();
                        saved_key = config.provider.credentials.api_key.expose().to_string();
                        saved_model = config.provider.model.model_id.clone();
                    }
                } else {
                    // Seed standard defaults if not active
                    if let Some(provider_enum) = id_to_provider(provider_id.as_str()) {
                        saved_base = provider_enum.capabilities().default_base_url.to_string();
                    }
                }

                win.set_api_base_url(saved_base.clone().into());
                win.set_api_key(saved_key.clone().into());
                win.set_active_model(saved_model.into());

                // Set fallback discovered models list from cache if it exists, otherwise empty
                let cached_models = {
                    let cache = discovered_models_cache().lock().unwrap();
                    cache.get(provider_id.as_str()).cloned().unwrap_or_default()
                };

                let slint_models: Vec<SharedString> = cached_models.into_iter().map(SharedString::from).collect();
                win.set_provider_models(ModelRc::from(Rc::new(VecModel::from(slint_models))));
                win.set_models_active_view(1); // Transition to setup form view

                // Auto-fetch available models if the API key is not empty (or if Ollama)
                if !saved_key.is_empty() || provider_id == "ollama" {
                    trigger_model_discovery(&win, &provider_id, &saved_base, &saved_key);
                }
            }
        }
    });

    // Handler 2: Triggered when the user clicks "Save & Activate" in the setup form
    window.on_provider_save_clicked({
        let weak_window = weak_window.clone();
        move |provider_id, api_base, api_key, selected_model| {
            println!("[operon-gui][settings] Saving provider setup for id={}", provider_id);
            
            let provider_enum = match id_to_provider(provider_id.as_str()) {
                Some(p) => p,
                None => {
                    eprintln!("[operon-gui][settings] Cannot save: Unknown provider ID: {}", provider_id);
                    return;
                }
            };

            // Build credentials
            let credentials = if !api_key.trim().is_empty() {
                operon_rs::ApiCredentials::with_key(api_key.as_str())
            } else {
                operon_rs::ApiCredentials::unauthenticated()
            };

            // Build model config
            // Use active model from form. We can resolve standard context sizes as fallback.
            let model_config = operon_rs::ModelConfig {
                model_id: selected_model.to_string(),
                context_window: 128_000, // Default fallback context window
                max_tokens: 4_096,       // Default fallback max tokens
            };

            let provider_config = operon_rs::ProviderConfig {
                provider: provider_enum,
                credentials,
                model: model_config,
                base_url_override: if api_base.trim().is_empty() { None } else { Some(api_base.to_string()) },
            };

            // Save the provider config using the backend's helper
            match operon_rs::save_provider(&provider_config) {
                Ok(_) => {
                    println!("[operon-gui][settings] Configuration saved successfully to config.toml");
                    if let Some(win) = weak_window.upgrade() {
                        // Refresh the providers list view with updated states
                        let providers = get_providers_list();
                        win.set_providers(ModelRc::from(Rc::new(VecModel::from(providers))));
                        win.set_models_active_view(0); // Return to list view
                    }
                }
                Err(e) => {
                    eprintln!("[operon-gui][settings] Failed to save configuration: {}", e);
                }
            }
        }
    });

    // Handler 3: Triggered when the user clicks "Reload" to fetch the model list dynamically
    window.on_provider_reload_clicked({
        let weak_window = weak_window.clone();
        move |provider_id, api_base, api_key| {
            if let Some(win) = weak_window.upgrade() {
                trigger_model_discovery(&win, &provider_id, &api_base, &api_key);
            }
        }
    });

    // Handler 4: Triggered automatically when the user modifies credentials (editing base URL or key)
    window.on_provider_credentials_changed({
        let weak_window = weak_window.clone();
        move |provider_id, api_base, api_key| {
            // Debounce/guard: Only run discovery if API key is populated and reasonably complete
            // (>= 15 characters) or for local auth-free Ollama setup.
            let is_valid_key = api_key.trim().len() >= 15 || provider_id == "ollama";
            if is_valid_key {
                if let Some(win) = weak_window.upgrade() {
                    trigger_model_discovery(&win, &provider_id, &api_base, &api_key);
                }
            }
        }
    });
}
