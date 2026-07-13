//! Model selection button controller.
//!
//! Handles displaying, triggering background API discovery, and switching
//! the active AI model for prompt execution.

use std::cell::RefCell;
use std::rc::Rc;
use slint::{ComponentHandle, ModelRc, VecModel, SharedString};

use crate::state::AppState;

/// Dynamic discovery of available models for the currently configured provider.
pub fn load_available_models(window: &crate::OperonWindow) {
    let window_weak = window.as_weak();
    
    tokio::spawn(async move {
        let run_load = async {
            let app_config = operon_rs::load()?;
            let provider_enum = app_config.provider.provider;
            let api_key = app_config.provider.credentials.api_key.clone();
            let api_base = app_config.provider.base_url_override.clone();
            
            // 1. Get standard models for this provider
            let mut models = match provider_enum {
                operon_rs::providers::Provider::Anthropic => vec![
                    "claude-3-5-sonnet-20241022".to_string(),
                    "claude-3-5-haiku-20241022".to_string(),
                    "claude-3-opus-20240229".to_string(),
                ],
                operon_rs::providers::Provider::OpenAI => vec![
                    "gpt-4o".to_string(),
                    "gpt-4-turbo".to_string(),
                    "gpt-4o-mini".to_string(),
                    "o1".to_string(),
                    "o1-mini".to_string(),
                ],
                operon_rs::providers::Provider::Gemini => vec![
                    "gemini-2.5-pro".to_string(),
                    "gemini-2.5-flash".to_string(),
                ],
                operon_rs::providers::Provider::DeepSeek => vec![
                    "deepseek-chat".to_string(),
                    "deepseek-coder".to_string(),
                ],
                operon_rs::providers::Provider::Groq => vec![
                    "llama3-70b-8192".to_string(),
                    "mixtral-8x7b-32768".to_string(),
                ],
                _ => vec![],
            };
            
            // Ensure currently active model is included and at the top
            let active_model = app_config.provider.model.model_id.clone();
            if !active_model.is_empty() {
                if let Some(pos) = models.iter().position(|m| m == &active_model) {
                    models.remove(pos);
                }
                models.insert(0, active_model.clone());
            }

            // Update UI thread-safely with standard models first
            let slint_models: Vec<SharedString> = models.iter().cloned().map(SharedString::from).collect();
            let active_model_ss = SharedString::from(active_model.clone());
            
            let window_weak_clone = window_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak_clone.upgrade() {
                    win.set_available_models(ModelRc::from(Rc::new(VecModel::from(slint_models))));
                    win.set_selected_model(active_model_ss);
                }
            });

            // 2. Perform live model discovery in the background if credentials exist
            let base_opt = api_base.as_deref();
            let has_key = !api_key.is_empty();
            let is_ollama = provider_enum == operon_rs::providers::Provider::Ollama;
            
            if has_key || is_ollama {
                if let Ok(result) = operon_rs::discover_models(provider_enum, api_key.expose(), base_opt).await {
                    let mut discovered_models: Vec<String> = result.models.into_iter().map(|m| m.model_id).collect();
                    if !discovered_models.is_empty() {
                        // Ensure active model is at the top
                        if !active_model.is_empty() {
                            if let Some(pos) = discovered_models.iter().position(|m| m == &active_model) {
                                discovered_models.remove(pos);
                            }
                            discovered_models.insert(0, active_model.clone());
                        }

                        let slint_discovered: Vec<SharedString> = discovered_models.into_iter().map(SharedString::from).collect();
                        let window_weak_clone = window_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(win) = window_weak_clone.upgrade() {
                                win.set_available_models(ModelRc::from(Rc::new(VecModel::from(slint_discovered))));
                            }
                        });
                    }
                }
            }
            anyhow::Ok(())
        }.await;
        if let Err(e) = run_load {
            eprintln!("[operon-gui][models] Failed to load available models list: {}", e);
        }
    });
}

/// Register model selector click callback.
pub fn wire_models(
    window: &crate::OperonWindow,
    _state: Rc<RefCell<AppState>>,
) {
    // Dynamically query and load the model options list on startup
    load_available_models(window);

    // Callback 1: Clicked model name toggles dropdown visibility in Slint
    let window_weak = window.as_weak();
    window.on_model_clicked(move || {
        println!("[operon-gui][input] Model button clicked (toggled in Slint).");
        if let Some(win) = window_weak.upgrade() {
            load_available_models(&win);
        }
    });

    // Callback 2: Selected a model from the dropdown list
    window.on_model_selected(move |selected_model| {
        println!("[operon-gui][input] Selected model from dropdown: {}", selected_model);
        
        let selected_model_str = selected_model.to_string();
        tokio::spawn(async move {
            let run_save = async {
                let mut app_config = operon_rs::load()?;
                app_config.provider.model.model_id = selected_model_str;
                operon_rs::save_provider(&app_config.provider)?;
                anyhow::Ok(())
            }.await;
            
            if let Err(e) = run_save {
                eprintln!("[operon-gui][models] Failed to save updated selected model: {}", e);
            }
        });
    });
}
