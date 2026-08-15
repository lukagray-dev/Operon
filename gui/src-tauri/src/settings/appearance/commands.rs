//! Appearance Settings Backend Commands.

use super::types::AppearanceSettingsDto;
use crate::settings::prefs::{GuiPrefs, ThinkingOrbStyle};

/// Retrieves current Appearance settings from disk (`~/.operon/gui_settings.toml`).
#[tauri::command]
pub async fn get_appearance_settings() -> Result<AppearanceSettingsDto, String> {
    let prefs = GuiPrefs::load();
    Ok(AppearanceSettingsDto {
        selected_theme: 0,
        selected_ui_scale: 1,
        compact_mode: false,
        smooth_animations: true,
        selected_thinking_orb: prefs.thinking_orb_style.to_index(),
        selected_ui_font: 0,
        selected_assistant_font: 0,
        selected_code_font: 0,
        cursor_blink_enabled: true,
    })
}

/// Saves Appearance settings to disk (`~/.operon/gui_settings.toml`).
#[tauri::command]
pub async fn save_appearance_settings(settings: AppearanceSettingsDto) -> Result<(), String> {
    let mut prefs = GuiPrefs::load();
    prefs.thinking_orb_style = ThinkingOrbStyle::from_index(settings.selected_thinking_orb);
    prefs.save()
}
