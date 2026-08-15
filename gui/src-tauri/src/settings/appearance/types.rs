//! Appearance Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// Appearance configuration matching Slint 1:1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceSettingsDto {
    pub selected_theme: i32, // 0 = Operon Dark, 1 = Midnight OLED, 2 = GitHub Dark, 3 = Tokyo Night
    pub selected_ui_scale: i32, // 0 = 80%, 1 = 100%, 2 = 120%, 3 = 140%, 4 = 160%
    pub compact_mode: bool,
    pub smooth_animations: bool,
    pub selected_thinking_orb: i32, // 0 = Breathing Aurora, 1 = Composing Prism, 2 = Solving Helix
    pub selected_ui_font: i32, // 0 = Open Sans, 1 = Inter, 2 = Roboto
    pub selected_assistant_font: i32, // 0 = Literata, 1 = Georgia, 2 = Merriweather
    pub selected_code_font: i32, // 0 = Kode Mono, 1 = Fira Code, 2 = JetBrains
    pub cursor_blink_enabled: bool,
}

impl Default for AppearanceSettingsDto {
    fn default() -> Self {
        Self {
            selected_theme: 0,
            selected_ui_scale: 1,
            compact_mode: false,
            smooth_animations: true,
            selected_thinking_orb: 1,
            selected_ui_font: 0,
            selected_assistant_font: 0,
            selected_code_font: 0,
            cursor_blink_enabled: true,
        }
    }
}
