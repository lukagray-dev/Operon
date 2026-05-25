// Models screen action handlers
// Handles: All Action::Models* variants
// These actions manage the models configuration screen (provider selection, setup form, model fetching)

use anyhow::Result;
use crate::events::action::Action;
use crate::state::AppState;
use tokio::sync::mpsc;

/// Handle models screen actions
/// Processes provider navigation, form input, model fetching, and configuration
pub async fn handle(
    action: Action,
    state: &mut AppState,
    tx: &mpsc::Sender<Action>,
) -> Result<()> {
    match action {
        Action::ModelsUp => {
            use crate::ui::screens::models::state::{ModelsStep, FetchStatus, Provider};
            match state.models.step {
                ModelsStep::ProviderList => {
                    // Navigate provider list
                    state.models.move_provider_up();
                }
                ModelsStep::Setup => {
                    // If models are fetched, prioritize model list navigation
                    if matches!(state.models.fetch_status, FetchStatus::Success) && !state.models.fetched_models.is_empty() {
                        // Navigate model list
                        state.models.move_model_up();
                    } else {
                        // Check if we're in a text input field - if so, forward to TextArea
                        let is_in_text_field = match state.models.selected_provider {
                            Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                state.models.focused_field == 0 // API key field
                            }
                            Some(Provider::Custom) => {
                                state.models.focused_field == 0 || state.models.focused_field == 2 // URL or API key
                            }
                            None => false,
                        };
                        
                        if is_in_text_field {
                            // Forward to TextArea (though Up/Down don't do much in single-line fields)
                            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                            let key_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
                            match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    let _ = state.models.api_key_input.input(key_event);
                                }
                                Some(Provider::Custom) => {
                                    if state.models.focused_field == 0 {
                                        let _ = state.models.base_url_input.input(key_event);
                                    } else {
                                        let _ = state.models.api_key_input.input(key_event);
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
        }
        Action::ModelsDown => {
            use crate::ui::screens::models::state::{ModelsStep, FetchStatus, Provider};
            match state.models.step {
                ModelsStep::ProviderList => {
                    // Navigate provider list
                    state.models.move_provider_down();
                }
                ModelsStep::Setup => {
                    // If models are fetched, prioritize model list navigation
                    if matches!(state.models.fetch_status, FetchStatus::Success) && !state.models.fetched_models.is_empty() {
                        // Navigate model list
                        state.models.move_model_down();
                    } else {
                        // Check if we're in a text input field - if so, forward to TextArea
                        let is_in_text_field = match state.models.selected_provider {
                            Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                state.models.focused_field == 0 // API key field
                            }
                            Some(Provider::Custom) => {
                                state.models.focused_field == 0 || state.models.focused_field == 2 // URL or API key
                            }
                            None => false,
                        };
                        
                        if is_in_text_field {
                            // Forward to TextArea (though Up/Down don't do much in single-line fields)
                            use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                            let key_event = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
                            match state.models.selected_provider {
                                Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                                    let _ = state.models.api_key_input.input(key_event);
                                }
                                Some(Provider::Custom) => {
                                    if state.models.focused_field == 0 {
                                        let _ = state.models.base_url_input.input(key_event);
                                    } else {
                                        let _ = state.models.api_key_input.input(key_event);
                                    }
                                }
                                None => {}
                            }
                        }
                    }
                }
            }
        }
        Action::ModelsLeft => {
            use crate::ui::screens::models::state::{ModelsStep, Provider};
            if matches!(state.models.step, ModelsStep::Setup) {
                // Check if we're on compat field - if so, toggle
                if matches!(state.models.selected_provider, Some(Provider::Custom))
                    && state.models.focused_field == 1 {
                    state.models.toggle_compat_mode();
                } else {
                    // Otherwise, forward to TextArea for cursor movement
                    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                    let key_event = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
                    match state.models.selected_provider {
                        Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                            let _ = state.models.api_key_input.input(key_event);
                        }
                        Some(Provider::Custom) => {
                            if state.models.focused_field == 0 {
                                let _ = state.models.base_url_input.input(key_event);
                            } else if state.models.focused_field == 2 {
                                let _ = state.models.api_key_input.input(key_event);
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        Action::ModelsRight => {
            use crate::ui::screens::models::state::{ModelsStep, Provider};
            if matches!(state.models.step, ModelsStep::Setup) {
                // Check if we're on compat field - if so, toggle
                if matches!(state.models.selected_provider, Some(Provider::Custom))
                    && state.models.focused_field == 1 {
                    state.models.toggle_compat_mode();
                } else {
                    // Otherwise, forward to TextArea for cursor movement
                    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
                    let key_event = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
                    match state.models.selected_provider {
                        Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                            let _ = state.models.api_key_input.input(key_event);
                        }
                        Some(Provider::Custom) => {
                            if state.models.focused_field == 0 {
                                let _ = state.models.base_url_input.input(key_event);
                            } else if state.models.focused_field == 2 {
                                let _ = state.models.api_key_input.input(key_event);
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        Action::ModelsConfirm => {
            use crate::ui::screens::models::state::{ModelsStep, Provider, FetchStatus};
            match state.models.step {
                ModelsStep::ProviderList => {
                    // Confirm provider selection and move to setup
                    state.models.confirm_provider();
                }
                ModelsStep::Setup => {
                    // Check if we're on the API key field - if so, trigger fetch
                    let is_on_api_key_field = match state.models.selected_provider {
                        Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                            // Only one field (API key), always field 0
                            state.models.focused_field == 0
                        }
                        Some(Provider::Custom) => {
                            // API key is field 2 (0=URL, 1=compat, 2=API key)
                            state.models.focused_field == 2
                        }
                        None => false,
                    };
                    
                    if is_on_api_key_field && !matches!(state.models.fetch_status, FetchStatus::Fetching) {
                        // Trigger fetch
                        state.models.start_fetch();
                        
                        // Spawn async mock fetch task
                        let provider = state.models.selected_provider;
                        let action_tx_clone = tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                            
                            let models = match provider {
                                Some(Provider::Anthropic) => vec![
                                    "claude-opus-4-5".to_string(),
                                    "claude-sonnet-4-5".to_string(),
                                    "claude-haiku-4-5".to_string(),
                                ],
                                Some(Provider::OpenAI) => vec![
                                    "gpt-4o".to_string(),
                                    "gpt-4o-mini".to_string(),
                                    "gpt-4-turbo".to_string(),
                                    "o1".to_string(),
                                    "o1-mini".to_string(),
                                ],
                                Some(Provider::Custom) => vec![
                                    "model-1".to_string(),
                                    "model-2".to_string(),
                                    "model-3".to_string(),
                                ],
                                None => vec![],
                            };
                            
                            let _ = action_tx_clone.send(Action::ModelsFetchComplete(models)).await;
                        });
                    } else if matches!(state.models.fetch_status, FetchStatus::Success) {
                        // If models are already fetched, Enter confirms the selected model
                        // TODO: Save configuration and return to Chat
                        state.set_active_screen(crate::state::screen::ActiveScreen::Chat);
                    }
                }
            }
        }
        Action::ModelsNextField => {
            use crate::ui::screens::models::state::ModelsStep;
            if matches!(state.models.step, ModelsStep::Setup) {
                state.models.next_field();
            }
        }
        Action::ModelsFetchModels => {
            use crate::ui::screens::models::state::{ModelsStep, FetchStatus, Provider};
            // Only fetch if on setup screen and not already fetching
            if matches!(state.models.step, ModelsStep::Setup) 
                && !matches!(state.models.fetch_status, FetchStatus::Fetching) {
                state.models.start_fetch();
                
                // Spawn async mock fetch task
                let provider = state.models.selected_provider;
                let action_tx_clone = tx.clone();
                tokio::spawn(async move {
                    // Mock delay (800ms)
                    tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                    
                    // Generate mock model list based on provider
                    let models = match provider {
                        Some(Provider::Anthropic) => vec![
                            "claude-opus-4-5".to_string(),
                            "claude-sonnet-4-5".to_string(),
                            "claude-haiku-4-5".to_string(),
                        ],
                        Some(Provider::OpenAI) => vec![
                            "gpt-4o".to_string(),
                            "gpt-4o-mini".to_string(),
                            "gpt-4-turbo".to_string(),
                            "o1".to_string(),
                            "o1-mini".to_string(),
                        ],
                        Some(Provider::Custom) => vec![
                            "model-1".to_string(),
                            "model-2".to_string(),
                            "model-3".to_string(),
                        ],
                        None => vec![],
                    };
                    
                    // Send completion action
                    let _ = action_tx_clone.send(Action::ModelsFetchComplete(models)).await;
                });
            }
        }
        Action::ModelsFetchComplete(models) => {
            // Complete the fetch operation with results
            state.models.complete_fetch(models);
        }
        Action::ModelsToggleCompat => {
            use crate::ui::screens::models::state::{ModelsStep, Provider};
            // Only toggle if on Custom provider setup and compat field is focused
            // Otherwise, Left/Right do nothing (they're not text input)
            if matches!(state.models.step, ModelsStep::Setup)
                && matches!(state.models.selected_provider, Some(Provider::Custom))
                && state.models.focused_field == 1 {
                state.models.toggle_compat_mode();
            }
        }
        Action::ModelsForwardKeyToInput(key_event) => {
            use crate::ui::screens::models::state::{ModelsStep, Provider};
            if matches!(state.models.step, ModelsStep::Setup) {
                // Forward key to the appropriate TextArea based on focused field
                match state.models.selected_provider {
                    Some(Provider::Anthropic) | Some(Provider::OpenAI) => {
                        // Only API key field (always focused)
                        let _ = state.models.api_key_input.input(key_event);
                    }
                    Some(Provider::Custom) => {
                        match state.models.focused_field {
                            0 => { let _ = state.models.base_url_input.input(key_event); } // Base URL field
                            1 => {} // Compat mode field (not text input)
                            2 => { let _ = state.models.api_key_input.input(key_event); } // API key field
                            _ => {}
                        }
                    }
                    None => {}
                }
            }
        }
        _ => {
            // Catch-all for safety (should never hit due to dispatch routing)
        }
    }
    
    Ok(())
}
