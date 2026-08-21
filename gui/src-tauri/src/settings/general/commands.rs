use super::types::GeneralSettingsDto;
use crate::settings::prefs::{CloseButtonAction, GuiPrefs};
use crate::shared::autostart;
use crate::shared::tray;
use tauri::{AppHandle, Emitter, Manager};

/// Retrieves current General settings from disk (`~/.operon/gui_settings.toml`).
#[tauri::command]
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
#[tauri::command]
pub async fn save_general_settings(
    settings: GeneralSettingsDto,
    app: AppHandle,
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

    // 2. Synchronize Windows startup registry
    let _ = autostart::set_autostart(settings.autostart_enabled, settings.start_minimized);

    // 3. Synchronize System Tray presence
    let _ = tray::update_system_tray(&app, settings.minimize_to_tray_enabled);

    // 4. Synchronize auto_approve state in AppState
    if let Some(state) = app.try_state::<crate::shared::AppState>() {
        if let Ok(mut lock) = state.state_lock.lock() {
            lock.auto_approve = settings.global_auto_approve_default;
        }
    }

    // 5. Emit event to all windows so main window input bar and messages controllers update instantly
    let _ = app.emit("operon://auto-approve-changed", settings.global_auto_approve_default);
    let _ = app.emit("operon://general-settings-changed", &settings);

    Ok(())
}
