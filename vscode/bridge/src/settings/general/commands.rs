use super::types::GeneralSettingsDto;
use crate::settings::prefs::{CloseButtonAction, GuiPrefs};
use crate::shared::AppState;
use std::sync::Arc;

/// Retrieves current General settings from disk (`~/.operon/gui_settings.toml`).
pub async fn get_general_settings() -> Result<GeneralSettingsDto, String> {
    let prefs = GuiPrefs::load();
    Ok(GeneralSettingsDto {
        autostart_enabled: prefs.autostart_enabled,
        minimize_to_tray_enabled: prefs.minimize_to_tray_enabled,
        start_minimized: prefs.start_minimized,
        close_button_action: prefs.close_button_action.to_index(),
        global_auto_approve_default: prefs.global_auto_approve_default,
        auto_scroll_stream: prefs.auto_scroll_stream,
        notify_on_permission_request: prefs.notify_on_permission_request,
        notify_on_response_complete: prefs.notify_on_response_complete,
        auto_collapse_reasoning_tools: prefs.auto_collapse_reasoning_tools,
        auto_update_checks: true,
        telemetry_enabled: false,
    })
}

/// Saves General settings to disk (`~/.operon/gui_settings.toml`) and applies system effects.
pub async fn save_general_settings(
    settings: GeneralSettingsDto,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut prefs = GuiPrefs::load();
    prefs.autostart_enabled = settings.autostart_enabled;
    prefs.minimize_to_tray_enabled = settings.minimize_to_tray_enabled;
    prefs.start_minimized = settings.start_minimized;
    prefs.close_button_action = CloseButtonAction::from_index(settings.close_button_action);
    prefs.global_auto_approve_default = settings.global_auto_approve_default;
    prefs.auto_scroll_stream = settings.auto_scroll_stream;
    prefs.notify_on_permission_request = settings.notify_on_permission_request;
    prefs.notify_on_response_complete = settings.notify_on_response_complete;
    prefs.auto_collapse_reasoning_tools = settings.auto_collapse_reasoning_tools;

    // 1. Save to disk
    prefs.save()?;

    // 2. Synchronize auto_approve state in AppState
    if let Ok(mut lock) = state.state_lock.lock() {
        lock.auto_approve = settings.global_auto_approve_default;
    }

    // 3. Emit notification event
    state
        .emit_event(
            "operon://auto-approve-changed",
            serde_json::json!(settings.global_auto_approve_default),
        )
        .await;

    Ok(())
}
