//! Persistence model for GUI-shell settings.
//!
//! Hey friend! This file manages persistent configuration options for the Operon GUI,
//! stored independently from `operon-rs`'s runtime config, alongside it under
//! `~/.operon/gui_settings.toml`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Action to perform when the user clicks the main window's Close (X) button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseButtonAction {
    /// Exit the application entirely.
    Exit,
    /// Hide the window to the system tray.
    MinimizeToTray,
}

impl Default for CloseButtonAction {
    fn default() -> Self {
        CloseButtonAction::Exit
    }
}

/// Animated orb style displayed in the collapsed work activity summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingOrbStyle {
    /// Ribbon-like composing animation.
    Composing,
    /// Lattice-style solving animation.
    Solving,
    /// Pulsing breathing-ring animation.
    Breathing,
}

impl Default for ThinkingOrbStyle {
    fn default() -> Self {
        ThinkingOrbStyle::Composing
    }
}

impl ThinkingOrbStyle {
    /// Converts the persisted enum into the Slint selector index.
    pub fn to_index(self) -> i32 {
        match self {
            ThinkingOrbStyle::Composing => 0,
            ThinkingOrbStyle::Solving => 1,
            ThinkingOrbStyle::Breathing => 2,
        }
    }

    /// Converts the Slint selector index into a persisted enum value.
    pub fn from_index(idx: i32) -> Self {
        match idx {
            1 => ThinkingOrbStyle::Solving,
            2 => ThinkingOrbStyle::Breathing,
            _ => ThinkingOrbStyle::Composing,
        }
    }
}

/// Helper function providing default value (true) for `auto_scroll_stream`.
fn default_auto_scroll_stream() -> bool {
    true
}

/// Helper function providing default value (true) for `notify_on_permission_request`.
fn default_notify_on_permission_request() -> bool {
    true
}

/// Helper function providing default value (false) for `notify_on_response_complete`.
fn default_notify_on_response_complete() -> bool {
    false
}

/// Helper function providing default value (false) for `auto_collapse_reasoning_tools`.
fn default_auto_collapse_reasoning_tools() -> bool {
    false
}

/// GUI preferences state saved on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiPrefs {
    /// Whether Operon is set to launch automatically on OS startup.
    #[serde(default)]
    pub autostart_enabled: bool,
    /// Whether system tray icon presence and minimize-to-tray features are active.
    #[serde(default)]
    pub minimize_to_tray_enabled: bool,
    /// Whether Operon starts minimized to system tray on boot.
    #[serde(default)]
    pub start_minimized: bool,
    /// Action taken when the main window close (X) button is clicked.
    #[serde(default)]
    pub close_button_action: CloseButtonAction,
    /// Which animated orb style is used for collapsed thinking/tool activity summaries.
    #[serde(default)]
    pub thinking_orb_style: ThinkingOrbStyle,
    /// Whether the chat viewport automatically scrolls to the bottom when new response tokens arrive from the model.
    #[serde(default = "default_auto_scroll_stream")]
    pub auto_scroll_stream: bool,
    /// Whether a desktop OS notification is sent when an agent asks for manual user permission confirmation.
    #[serde(default = "default_notify_on_permission_request")]
    pub notify_on_permission_request: bool,
    /// Whether a desktop OS notification is sent when an agent response turn finishes.
    #[serde(default = "default_notify_on_response_complete")]
    pub notify_on_response_complete: bool,
    /// Whether reasoning/thinking and tool activity summary pills automatically collapse.
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
            None => {
                tracing::warn!("[operon-gui][prefs] Could not determine user config directory.");
                return Self::default();
            }
        };

        if !path.exists() {
            return Self::default();
        }

        match fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<GuiPrefs>(&content) {
                Ok(prefs) => prefs,
                Err(error) => {
                    tracing::warn!(
                        "[operon-gui][prefs] Failed to parse TOML settings from {}: {error:#}",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(error) => {
                tracing::warn!(
                    "[operon-gui][prefs] Failed to read settings file at {}: {error:#}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Saves the current preferences to disk.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_file_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        tracing::info!(
            "[operon-gui][prefs] Settings saved successfully to {}",
            path.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gui_prefs_defaults() {
        let prefs = GuiPrefs::default();
        assert!(!prefs.autostart_enabled);
        assert!(!prefs.minimize_to_tray_enabled);
        assert!(!prefs.start_minimized);
        assert_eq!(prefs.close_button_action, CloseButtonAction::Exit);
        assert_eq!(prefs.thinking_orb_style, ThinkingOrbStyle::Composing);
        assert!(prefs.auto_scroll_stream);
        assert!(prefs.notify_on_permission_request);
        assert!(!prefs.notify_on_response_complete);
    }

    #[test]
    fn test_toml_serialization_round_trip() {
        let original = GuiPrefs {
            autostart_enabled: true,
            minimize_to_tray_enabled: true,
            start_minimized: false,
            close_button_action: CloseButtonAction::Exit,
            thinking_orb_style: ThinkingOrbStyle::Solving,
            auto_scroll_stream: false,
            notify_on_permission_request: false,
            notify_on_response_complete: true,
        };

        let serialized = toml::to_string(&original).expect("serialization failed");
        assert!(serialized.contains("autostart_enabled = true"));
        assert!(serialized.contains("close_button_action = \"exit\""));
        assert!(serialized.contains("thinking_orb_style = \"solving\""));
        assert!(serialized.contains("auto_scroll_stream = false"));
        assert!(serialized.contains("notify_on_permission_request = false"));
        assert!(serialized.contains("notify_on_response_complete = true"));

        let deserialized: GuiPrefs = toml::from_str(&serialized).expect("deserialization failed");
        assert_eq!(deserialized.autostart_enabled, original.autostart_enabled);
        assert_eq!(
            deserialized.minimize_to_tray_enabled,
            original.minimize_to_tray_enabled
        );
        assert_eq!(deserialized.start_minimized, original.start_minimized);
        assert_eq!(
            deserialized.close_button_action,
            original.close_button_action
        );
        assert_eq!(deserialized.thinking_orb_style, original.thinking_orb_style);
        assert_eq!(deserialized.auto_scroll_stream, original.auto_scroll_stream);
        assert_eq!(
            deserialized.notify_on_permission_request,
            original.notify_on_permission_request
        );
        assert_eq!(
            deserialized.notify_on_response_complete,
            original.notify_on_response_complete
        );
    }

    #[test]
    fn test_toml_deserialization_partial_defaults() {
        let partial_toml = r#"
            autostart_enabled = true
        "#;

        let prefs: GuiPrefs = toml::from_str(partial_toml).expect("parse partial toml");
        assert!(prefs.autostart_enabled);
        assert!(!prefs.minimize_to_tray_enabled);
        assert!(!prefs.start_minimized);
        assert_eq!(prefs.close_button_action, CloseButtonAction::Exit);
        assert_eq!(prefs.thinking_orb_style, ThinkingOrbStyle::Composing);
        assert!(prefs.auto_scroll_stream);
        assert!(prefs.notify_on_permission_request);
        assert!(!prefs.notify_on_response_complete);
    }
}
