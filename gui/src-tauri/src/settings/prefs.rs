//! GUI Settings Persistence Manager (`~/.operon/gui_settings.toml`).
//
// 1:1 match with Slint GuiPrefs architecture:
// - Stores application startup, tray, notifications, stream scrolling, and thinking orb preferences.
// - Persists cleanly to ~/.operon/gui_settings.toml.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Action taken when the main window close button is clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CloseButtonAction {
    #[default]
    Exit,
    MinimizeToTray,
}

impl CloseButtonAction {
    pub fn from_index(idx: i32) -> Self {
        match idx {
            1 => Self::MinimizeToTray,
            _ => Self::Exit,
        }
    }

    pub fn to_index(self) -> i32 {
        match self {
            Self::Exit => 0,
            Self::MinimizeToTray => 1,
        }
    }
}

/// Visual style of the thinking / reasoning animated orb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingOrbStyle {
    #[default]
    Composing,
    Shaping,
    Working,
    Connecting,
}

impl ThinkingOrbStyle {
    pub fn from_index(idx: i32) -> Self {
        match idx {
            0 => Self::Composing,
            1 => Self::Shaping,
            2 => Self::Working,
            3 => Self::Connecting,
            _ => Self::Composing,
        }
    }

    pub fn to_index(self) -> i32 {
        match self {
            Self::Composing => 0,
            Self::Shaping => 1,
            Self::Working => 2,
            Self::Connecting => 3,
        }
    }
}

fn default_auto_scroll_stream() -> bool {
    true
}

fn default_notify_on_permission_request() -> bool {
    true
}

fn default_notify_on_response_complete() -> bool {
    false
}

fn default_auto_collapse_reasoning_tools() -> bool {
    false
}

fn default_true() -> bool {
    true
}

/// GUI preferences state saved on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiPrefs {
    #[serde(default)]
    pub autostart_enabled: bool,
    #[serde(default)]
    pub minimize_to_tray_enabled: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub close_button_action: CloseButtonAction,
    #[serde(default)]
    pub thinking_orb_style: ThinkingOrbStyle,
    #[serde(default)]
    pub global_auto_approve_default: bool,
    #[serde(default = "default_auto_scroll_stream")]
    pub auto_scroll_stream: bool,
    #[serde(default = "default_notify_on_permission_request")]
    pub notify_on_permission_request: bool,
    #[serde(default = "default_notify_on_response_complete")]
    pub notify_on_response_complete: bool,
    #[serde(default = "default_auto_collapse_reasoning_tools")]
    pub auto_collapse_reasoning_tools: bool,
    #[serde(default = "default_true")]
    pub auto_update_checks: bool,

    // Appearance: Markdown & Code block settings
    #[serde(default)]
    pub code_block_theme: i32, // 0 = GitHub Dark, 1 = Midnight OLED, 2 = Tokyo Night, 3 = Monokai
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default = "default_true")]
    pub highlight_inline_code: bool,
    #[serde(default)]
    pub table_theme: i32, // 0 = GitHub Dark, 1 = Modern Minimal, 2 = Zebra Striped, 3 = Boxed Grid

    // Appearance: Typography & Font Choices
    #[serde(default)]
    pub selected_ui_font: i32, // 0 = Open Sans, 1 = Inter, 2 = Roboto
    #[serde(default)]
    pub selected_assistant_font: i32, // 0 = Literata, 1 = Lora, 2 = Merriweather
    #[serde(default)]
    pub selected_code_font: i32, // 0 = Kode Mono, 1 = JetBrains Mono, 2 = Fira Code

    // Appearance: Thinking Orb Controls
    #[serde(default = "default_orb_speed")]
    pub orb_speed: i32, // 0 = 1.5x, 1 = 3.0x (default), 2 = 4.5x
    #[serde(default = "default_show_live_orb")]
    pub show_live_orb: bool,
}

fn default_orb_speed() -> i32 {
    1
}

fn default_show_live_orb() -> bool {
    true
}

impl Default for GuiPrefs {
    fn default() -> Self {
        Self {
            autostart_enabled: false,
            minimize_to_tray_enabled: false,
            start_minimized: false,
            close_button_action: CloseButtonAction::default(),
            thinking_orb_style: ThinkingOrbStyle::default(),
            global_auto_approve_default: false,
            auto_scroll_stream: true,
            notify_on_permission_request: true,
            notify_on_response_complete: false,
            auto_collapse_reasoning_tools: false,
            auto_update_checks: true,
            code_block_theme: 0,
            show_line_numbers: true,
            highlight_inline_code: true,
            table_theme: 0,
            selected_ui_font: 0,
            selected_assistant_font: 0,
            selected_code_font: 0,
            orb_speed: 1,
            show_live_orb: true,
        }
    }
}

impl GuiPrefs {
    /// Returns the standard config file path: `~/.operon/gui_settings.toml`.
    pub fn config_file_path() -> Option<PathBuf> {
        dirs::home_dir().map(|dir| dir.join(".operon").join("gui_settings.toml"))
    }

    /// Loads settings from disk. Returns `Default::default()` on missing file or parse failure.
    pub fn load() -> Self {
        let path = match Self::config_file_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str::<GuiPrefs>(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Saves the current preferences to disk.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_file_path()
            .ok_or_else(|| "Could not determine user config directory".to_string())?;

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let content = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
}
