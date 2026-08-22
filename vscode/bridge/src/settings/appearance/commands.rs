use super::types::AppearanceSettingsDto;
use crate::settings::prefs::{GuiPrefs, ThinkingOrbStyle};
use crate::shared::AppState;
use std::sync::Arc;

/// Retrieves current Appearance settings from disk (`~/.operon/gui_settings.toml`).
pub async fn get_appearance_settings() -> Result<AppearanceSettingsDto, String> {
    let prefs = GuiPrefs::load();
    Ok(AppearanceSettingsDto {
        selected_theme: 0,
        selected_ui_scale: 1,
        compact_mode: false,
        smooth_animations: true,
        selected_thinking_orb: prefs.thinking_orb_style.to_index(),
        selected_ui_font: prefs.selected_ui_font,
        selected_assistant_font: prefs.selected_assistant_font,
        selected_code_font: prefs.selected_code_font,
        code_block_theme: prefs.code_block_theme,
        show_line_numbers: prefs.show_line_numbers,
        highlight_inline_code: prefs.highlight_inline_code,
        table_theme: prefs.table_theme,
        orb_speed: prefs.orb_speed,
        show_live_orb: prefs.show_live_orb,
    })
}

/// Saves Appearance settings to disk (`~/.operon/gui_settings.toml`) and emits live update event.
pub async fn save_appearance_settings(
    settings: AppearanceSettingsDto,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let mut prefs = GuiPrefs::load();
    prefs.thinking_orb_style = ThinkingOrbStyle::from_index(settings.selected_thinking_orb);
    prefs.selected_ui_font = settings.selected_ui_font;
    prefs.selected_assistant_font = settings.selected_assistant_font;
    prefs.selected_code_font = settings.selected_code_font;
    prefs.code_block_theme = settings.code_block_theme;
    prefs.show_line_numbers = settings.show_line_numbers;
    prefs.highlight_inline_code = settings.highlight_inline_code;
    prefs.table_theme = settings.table_theme;
    prefs.orb_speed = settings.orb_speed;
    prefs.show_live_orb = settings.show_live_orb;
    prefs.save()?;

    state
        .emit_event(
            "operon://appearance-changed",
            serde_json::to_value(&settings).unwrap_or_default(),
        )
        .await;

    Ok(())
}
