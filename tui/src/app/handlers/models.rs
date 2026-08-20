// models.rs — Models & AI Providers screen action handlers for Operon TUI.
//
// ZERO BUSINESS LOGIC IN FRONTEND:
// The TUI is strictly a presentation shell over `operon-rs`.
// - Model auto-discovery is dispatched to `operon_rs::discover_models(...)`.
// - Configuration persistence is executed via `operon_rs::save_provider(...)`.
// - Active session state in AppState is synchronized immediately after saving.

use crate::events::action::Action;
use crate::state::AppState;
use crate::ui::screens::models::state::{FetchStatus, ModelsStep, SaveStatus, SetupField};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

/// Processes all Models-related actions triggered by keyboard events or background tasks.
pub async fn handle(action: Action, state: &mut AppState, tx: &mpsc::Sender<Action>) -> Result<()> {
    match action {
        // ─────────────────────────────────────────────────────────────────────
        // Navigation: Up
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsUp => match state.models.step {
            ModelsStep::ProviderList => {
                state.models.move_provider_up();
            }
            ModelsStep::Setup => match state.models.focused_field {
                SetupField::DiscoveredModelList => {
                    state.models.move_model_up();
                }
                SetupField::BaseUrl => {
                    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                    let _ = state.models.base_url_input.input(key);
                }
                SetupField::ApiKey => {
                    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                    let _ = state.models.api_key_input.input(key);
                }
                SetupField::CustomModel => {
                    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                    let _ = state.models.custom_model_input.input(key);
                }
                _ => {}
            },
        },

        // ─────────────────────────────────────────────────────────────────────
        // Navigation: Down
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsDown => match state.models.step {
            ModelsStep::ProviderList => {
                state.models.move_provider_down();
            }
            ModelsStep::Setup => match state.models.focused_field {
                SetupField::DiscoveredModelList => {
                    state.models.move_model_down();
                }
                SetupField::BaseUrl => {
                    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                    let _ = state.models.base_url_input.input(key);
                }
                SetupField::ApiKey => {
                    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                    let _ = state.models.api_key_input.input(key);
                }
                SetupField::CustomModel => {
                    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                    let _ = state.models.custom_model_input.input(key);
                }
                _ => {}
            },
        },

        // ─────────────────────────────────────────────────────────────────────
        // Cursor movement: Left
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsLeft => {
            if state.models.step == ModelsStep::Setup {
                let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
                match state.models.focused_field {
                    SetupField::BaseUrl => {
                        let _ = state.models.base_url_input.input(key);
                    }
                    SetupField::ApiKey => {
                        let _ = state.models.api_key_input.input(key);
                    }
                    SetupField::CustomModel => {
                        let _ = state.models.custom_model_input.input(key);
                    }
                    _ => {}
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Cursor movement: Right
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsRight => {
            if state.models.step == ModelsStep::Setup {
                let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
                match state.models.focused_field {
                    SetupField::BaseUrl => {
                        let _ = state.models.base_url_input.input(key);
                    }
                    SetupField::ApiKey => {
                        let _ = state.models.api_key_input.input(key);
                    }
                    SetupField::CustomModel => {
                        let _ = state.models.custom_model_input.input(key);
                    }
                    _ => {}
                }
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Confirmation / Selection (Enter)
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsConfirm => match state.models.step {
            ModelsStep::ProviderList => {
                state.models.confirm_provider();
            }
            ModelsStep::Setup => match state.models.focused_field {
                SetupField::FetchButton => {
                    trigger_fetch_models(state, tx).await;
                }
                SetupField::DiscoveredModelList => {
                    state.models.select_discovered_model();
                }
                SetupField::SaveButton => {
                    trigger_save_provider(state, tx).await;
                }
                SetupField::ApiKey => {
                    // Enter on API Key triggers model discovery if not yet fetched
                    if matches!(state.models.fetch_status, FetchStatus::Idle) {
                        trigger_fetch_models(state, tx).await;
                    } else {
                        state.models.next_field();
                    }
                }
                SetupField::CustomModel => {
                    // Enter on Custom Model triggers save
                    trigger_save_provider(state, tx).await;
                }
                SetupField::BaseUrl => {
                    state.models.next_field();
                }
            },
        },

        // ─────────────────────────────────────────────────────────────────────
        // Field Navigation (Tab / Shift+Tab)
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsNextField => {
            if state.models.step == ModelsStep::Setup {
                state.models.next_field();
            }
        }
        Action::ModelsPrevField => {
            if state.models.step == ModelsStep::Setup {
                state.models.prev_field();
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // API Key Visibility Toggle (F2)
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsToggleKeyVisibility => {
            if state.models.step == ModelsStep::Setup {
                state.models.toggle_api_key_visibility();
            }
        }

        // ─────────────────────────────────────────────────────────────────────
        // Real-Time Model Discovery
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsFetchModels => {
            if state.models.step == ModelsStep::Setup {
                trigger_fetch_models(state, tx).await;
            }
        }
        Action::ModelsFetchComplete(result) => match result {
            Ok(models) => {
                state.models.complete_fetch(models);
            }
            Err(err) => {
                state.models.fail_fetch(err);
            }
        },

        // ─────────────────────────────────────────────────────────────────────
        // Configuration Persistence
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsSaveProvider => {
            if state.models.step == ModelsStep::Setup {
                trigger_save_provider(state, tx).await;
            }
        }
        Action::ModelsSaveComplete(result) => match result {
            Ok(()) => {
                state.models.save_status = SaveStatus::Success;

                // Refresh backend state in models screen and session context
                state.models.refresh_from_backend();
                state.session_mut().refresh_from_backend();
            }
            Err(err) => {
                state.models.save_status = SaveStatus::Error(err);
            }
        },

        // ─────────────────────────────────────────────────────────────────────
        // Forward Keystrokes to Focused TextArea
        // ─────────────────────────────────────────────────────────────────────
        Action::ModelsForwardKeyToInput(key_event) => {
            if state.models.step == ModelsStep::Setup {
                match state.models.focused_field {
                    SetupField::BaseUrl => {
                        let _ = state.models.base_url_input.input(key_event);
                    }
                    SetupField::ApiKey => {
                        let _ = state.models.api_key_input.input(key_event);
                    }
                    SetupField::CustomModel => {
                        let _ = state.models.custom_model_input.input(key_event);
                    }
                    _ => {}
                }
            }
        }

        _ => {}
    }

    Ok(())
}

/// Helper to trigger asynchronous model discovery via operon_rs::discover_models.
async fn trigger_fetch_models(state: &mut AppState, tx: &mpsc::Sender<Action>) {
    let provider = match state.models.selected_provider {
        Some(p) => p,
        None => return,
    };

    if matches!(state.models.fetch_status, FetchStatus::Fetching) {
        return;
    }

    state.models.start_fetch();

    let api_key = state.models.api_key_input.lines().join("").trim().to_string();
    let base_url = state.models.base_url_input.lines().join("").trim().to_string();

    let base_override = if base_url.is_empty() {
        None
    } else {
        Some(base_url)
    };

    let action_tx = tx.clone();

    tokio::spawn(async move {
        let result = operon_rs::discover_models(
            provider,
            &api_key,
            base_override.as_deref(),
        )
        .await;

        let mapped = match result {
            Ok(discovery) => Ok(discovery.models),
            Err(err) => Err(format!("{:#}", err)),
        };

        let _ = action_tx.send(Action::ModelsFetchComplete(mapped)).await;
    });
}

/// Helper to trigger asynchronous configuration save via operon_rs::save_provider.
async fn trigger_save_provider(state: &mut AppState, tx: &mpsc::Sender<Action>) {
    let provider = match state.models.selected_provider {
        Some(p) => p,
        None => return,
    };

    state.models.save_status = SaveStatus::Saving;

    let api_key = state.models.api_key_input.lines().join("").trim().to_string();
    let base_url = state.models.base_url_input.lines().join("").trim().to_string();
    let model_config = state.models.resolve_model_config(provider);

    let credentials = if !api_key.is_empty() {
        operon_rs::ApiCredentials::with_key(api_key.as_str())
    } else {
        operon_rs::ApiCredentials::unauthenticated()
    };

    let provider_config = operon_rs::ProviderConfig {
        provider,
        credentials,
        model: model_config,
        base_url_override: if base_url.is_empty() {
            None
        } else {
            Some(base_url)
        },
    };

    let action_tx = tx.clone();

    tokio::spawn(async move {
        let res = operon_rs::save_provider(&provider_config).map_err(|e| e.to_string());
        let _ = action_tx.send(Action::ModelsSaveComplete(res)).await;
    });
}
