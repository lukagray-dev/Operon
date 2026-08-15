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
    Breathing,
    #[default]
    Composing,
    Solving,
}

impl ThinkingOrbStyle {
    pub fn from_index(idx: i32) -> Self {
        match idx {
            0 => Self::Breathing,
            1 => Self::Composing,
            2 => Self::Solving,
            _ => Self::Composing,
        }
    }

    pub fn to_index(self) -> i32 {
        match self {
            Self::Breathing => 0,
            Self::Composing => 1,
            Self::Solving => 2,
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
    #[serde(default = "default_auto_scroll_stream")]
    pub auto_scroll_stream: bool,
    #[serde(default = "default_notify_on_permission_request")]
    pub notify_on_permission_request: bool,
    #[serde(default = "default_notify_on_response_complete")]
    pub notify_on_response_complete: bool,
    #[serde(default = "default_auto_collapse_reasoning_tools")]
    pub auto_collapse_reasoning_tools: bool,
}

impl Default for GuiPrefs {
    fn default() -> Self {
        Self {
            autostart_enabled: false,
            minimize_to_tray_enabled: false,
            start_minimized: false,
            close_button_action: CloseButtonAction::default(),
            thinking_orb_style: ThinkingOrbStyle::default(),
            auto_scroll_stream: true,
            notify_on_permission_request: true,
            notify_on_response_complete: false,
            auto_collapse_reasoning_tools: false,
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
