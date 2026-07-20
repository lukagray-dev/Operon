//! Model selection button controller.
//!
//! Handles displaying, triggering background API discovery, and switching
//! the active AI model for prompt execution.

use crate::state::AppState;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

/// Global thread-safe cache of discovered model metadata.
pub static DISCOVERED_MODELS: Mutex<Vec<operon_rs::DiscoveredModel>> = Mutex::new(Vec::new());

/// Dynamic discovery of available models for the currently configured provider.
pub fn load_available_models(window: &crate::OperonWindow) {
    let window_weak = window.as_weak();

    tokio::spawn(async move {
        let run_load = async {
            let app_config = operon_rs::load()?;
            let provider_enum = app_config.provider.provider;
            let api_key = app_config.provider.credentials.api_key.clone();
            let api_base = app_config.provider.base_url_override.clone();

            let active_model = app_config.provider.model.model_id.clone();

            // Start with only the currently configured active model
            let models = vec![active_model.clone()];

            // Update UI thread-safely with the active model first
            let slint_models: Vec<SharedString> =
                models.iter().cloned().map(SharedString::from).collect();
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
                if let Ok(result) =
                    operon_rs::discover_models(provider_enum, api_key.expose(), base_opt).await
                {
                    let mut discovered_models_ids: Vec<String> =
                        result.models.iter().map(|m| m.model_id.clone()).collect();
                    if !discovered_models_ids.is_empty() {
                        // Cache the full discovered models in global static cache
                        {
                            *DISCOVERED_MODELS.lock().unwrap() = result.models.clone();
                        }

                        // Ensure active model is at the top
                        if !active_model.is_empty() {
                            if let Some(pos) = discovered_models_ids
                                .iter()
                                .position(|m| m == &active_model)
                            {
                                discovered_models_ids.remove(pos);
                            }
                            discovered_models_ids.insert(0, active_model.clone());
                        }

                        let slint_discovered: Vec<SharedString> = discovered_models_ids
                            .into_iter()
                            .map(SharedString::from)
                            .collect();
                        let window_weak_clone = window_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(win) = window_weak_clone.upgrade() {
                                win.set_available_models(ModelRc::from(Rc::new(VecModel::from(
                                    slint_discovered,
                                ))));
                            }
                        });
                    }
                }
            }
            anyhow::Ok(())
        }
        .await;
        if let Err(e) = run_load {
            eprintln!(
                "[operon-gui][models] Failed to load available models list: {}",
                e
            );
        }
    });
}

/// Register model selector click callback.
pub fn wire_models(window: &crate::OperonWindow, _state: Rc<RefCell<AppState>>) {
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
    let window_weak = window.as_weak();
    window.on_model_selected(move |selected_model| {
        println!(
            "[operon-gui][input] Selected model from dropdown: {}",
            selected_model
        );

        let selected_model_str = selected_model.to_string();
        let win_weak = window_weak.clone();

        tokio::spawn(async move {
            let run_save = async {
                let mut app_config = operon_rs::load()?;

                // Read context window and max tokens from global cached models
                let (context_window, max_tokens) = {
                    let cached = DISCOVERED_MODELS.lock().unwrap();
                    if let Some(m) = cached.iter().find(|m| m.model_id == selected_model_str) {
                        (m.context_window, m.max_tokens)
                    } else {
                        // Fallback to configured values if not in discovered models cache
                        (
                            app_config.provider.model.context_window,
                            app_config.provider.model.max_tokens,
                        )
                    }
                };

                app_config.provider.model.model_id = selected_model_str;
                app_config.provider.model.context_window = context_window;
                app_config.provider.model.max_tokens = max_tokens;

                operon_rs::save_provider(&app_config.provider)?;

                // Update UI thread-safely with new context size
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = win_weak.upgrade() {
                        win.set_tokens_total(context_window as i32);
                        let used = win.get_tokens_used();
                        let utilization = if context_window > 0 {
                            used as f32 / context_window as f32
                        } else {
                            0.0
                        };
                        win.set_context_usage(utilization);
                        let context_text = crate::main_content::input::context::format_tokens(
                            used,
                            context_window as i32,
                        );
                        win.set_context_text(context_text.into());
                    }
                });

                anyhow::Ok(())
            }
            .await;

            if let Err(e) = run_save {
                eprintln!(
                    "[operon-gui][models] Failed to save updated selected model: {}",
                    e
                );
            }
        });
    });
}
