use tauri::{AppHandle, Emitter, State};

use super::types::{ContextUsageDto, ModelOptionDto, PendingAttachmentDto};
use crate::shared::AppState;

/// Retrieves the active configured model and queries dynamically discovered models from the provider,
/// including their supported reasoning levels as fetched from the provider API.
#[tauri::command]
pub async fn get_available_models() -> Result<Vec<ModelOptionDto>, String> {
    let app_config = operon_rs::load().map_err(|e| e.to_string())?;
    let active_model_id = app_config.provider.model.model_id.clone();
    let mut context_window = app_config.provider.model.context_window;
    let selected_reasoning = app_config.provider.model.reasoning_effort.clone();

    let mut models = Vec::new();
    let mut active_reasoning_levels = Vec::new();

    let provider_enum = app_config.provider.provider;
    let api_key = app_config.provider.credentials.api_key.clone();
    let api_base = app_config.provider.base_url_override.clone();

    let has_key = !api_key.is_empty();
    let is_ollama = provider_enum == operon_rs::providers::Provider::Ollama;

    if has_key || is_ollama {
        if let Ok(result) =
            operon_rs::discover_models(provider_enum, api_key.expose(), api_base.as_deref()).await
        {
            for discovered in result.models {
                if discovered.model_id == active_model_id {
                    active_reasoning_levels = discovered.reasoning_levels.clone();
                    context_window = discovered.context_window;
                } else {
                    models.push(ModelOptionDto {
                        id: discovered.model_id.clone(),
                        name: discovered.model_id.clone(),
                        is_active: false,
                        context_window: discovered.context_window,
                        reasoning_levels: discovered.reasoning_levels,
                        selected_reasoning: None,
                    });
                }
            }
        }
    }

    // Insert active model at the very front of the list
    models.insert(
        0,
        ModelOptionDto {
            id: active_model_id.clone(),
            name: active_model_id.clone(),
            is_active: true,
            context_window,
            reasoning_levels: active_reasoning_levels,
            selected_reasoning,
        },
    );

    Ok(models)
}

/// Sets the active model ID, optional reasoning level, and context window in configuration and saves provider config.
#[tauri::command]
pub async fn select_model(
    model_id: String,
    reasoning: Option<String>,
    context_window: Option<usize>,
) -> Result<(), String> {
    let mut app_config = operon_rs::load().map_err(|e| e.to_string())?;
    app_config.provider.model.model_id = model_id.clone();
    app_config.provider.model.reasoning_effort =
        reasoning.filter(|r| !r.trim().is_empty() && r != "Disabled");

    if let Some(cw) = context_window {
        app_config.provider.model.context_window = cw;
    } else {
        let cw = operon_rs::lookup_context_window(&model_id);
        app_config.provider.model.context_window = cw;
    }

    let _ = operon_rs::config::save_provider(&app_config.provider);
    Ok(())
}

/// Toggles auto-approve permissions mode.
#[tauri::command]
pub async fn toggle_auto_approve(
    enabled: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<bool, String> {
    if let Ok(mut lock) = state.state_lock.lock() {
        lock.auto_approve = enabled;
    }
    let mut prefs = crate::settings::prefs::GuiPrefs::load();
    prefs.global_auto_approve_default = enabled;
    let _ = prefs.save();

    let _ = app.emit("operon://auto-approve-changed", enabled);
    Ok(enabled)
}

/// Opens native file dialog to pick files or images for prompt context attachment.
#[tauri::command]
pub async fn pick_attachments_dialog() -> Result<Vec<PendingAttachmentDto>, String> {
    let picked_files = rfd::FileDialog::new()
        .set_title("Attach Files or Images")
        .add_filter(
            "All Supported",
            &[
                "png", "jpg", "jpeg", "webp", "gif", "txt", "md", "rs", "ts", "js", "py", "json",
                "toml", "css", "html", "pdf",
            ],
        )
        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
        .add_filter(
            "Code & Text",
            &[
                "txt", "md", "rs", "ts", "js", "py", "json", "toml", "css", "html",
            ],
        )
        .pick_files();

    let mut results = Vec::new();
    if let Some(files) = picked_files {
        for path_buf in files {
            let path_str = path_buf.to_string_lossy().to_string();
            let file_name = path_buf
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path_str)
                .to_string();

            let ext = path_buf
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let is_image = matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif");
            let size_bytes = std::fs::metadata(&path_buf).map(|m| m.len()).unwrap_or(0);

            results.push(PendingAttachmentDto {
                path: path_str,
                file_name,
                is_image,
                size_bytes,
            });
        }
    }

    Ok(results)
}

/// Formats real token usage from session database against configured model context window.
#[tauri::command]
pub async fn get_context_window_info(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<ContextUsageDto, String> {
    let app_config = operon_rs::load().ok();
    let tokens_total = app_config
        .as_ref()
        .map(|c| c.provider.model.context_window)
        .unwrap_or(0);

    let resolved_session_id = session_id.or_else(|| {
        state
            .state_lock
            .lock()
            .ok()
            .and_then(|s| s.active_session_id.clone())
    });

    let tokens_used = if let Some(ref sid) = resolved_session_id {
        if let Ok(paths) = operon_rs::config::OperonPaths::resolve() {
            let db_path = paths.session_db(sid);
            if db_path.exists() {
                if let Ok(store) = operon_rs::session::store::SessionStore::open(&db_path).await {
                    store
                        .get_last_token_count(sid)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or(0)
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    };

    let percentage = if tokens_total > 0 {
        (tokens_used as f32 / tokens_total as f32) * 100.0
    } else {
        0.0
    };

    let formatted = if tokens_total > 0 {
        let total_str = if tokens_total >= 1_000_000 {
            let m = tokens_total as f32 / 1_000_000.0;
            if m.fract() == 0.0 {
                format!("{:.0}M", m)
            } else {
                format!("{:.1}M", m)
            }
        } else if tokens_total >= 1_000 {
            format!("{}k", tokens_total / 1_000)
        } else {
            format!("{}", tokens_total)
        };

        if tokens_used >= 1_000_000 {
            format!("{:.1}M / {}", tokens_used as f32 / 1_000_000.0, total_str)
        } else if tokens_used >= 1_000 {
            format!("{:.1}k / {}", tokens_used as f32 / 1000.0, total_str)
        } else {
            format!("{} / {}", tokens_used, total_str)
        }
    } else if tokens_used > 0 {
        format!("{}", tokens_used)
    } else {
        String::new()
    };

    Ok(ContextUsageDto {
        tokens_used,
        tokens_total,
        percentage,
        formatted,
    })
}
