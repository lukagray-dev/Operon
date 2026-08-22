//! General Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// General application configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettingsDto {
    pub autostart_enabled: bool,
    pub minimize_to_tray_enabled: bool,
    pub start_minimized: bool,
    pub close_button_action: i32, // 0 = Exit App, 1 = Minimize to Tray
    pub global_auto_approve_default: bool,
    pub auto_scroll_stream: bool,
    pub notify_on_permission_request: bool,
    pub notify_on_response_complete: bool,
    pub auto_collapse_reasoning_tools: bool,
    pub auto_update_checks: bool,
    pub telemetry_enabled: bool,
}

impl Default for GeneralSettingsDto {
    fn default() -> Self {
        Self {
            autostart_enabled: false,
            minimize_to_tray_enabled: false,
            start_minimized: false,
            close_button_action: 0,
            global_auto_approve_default: false,
            auto_scroll_stream: true,
            notify_on_permission_request: true,
            notify_on_response_complete: false,
            auto_collapse_reasoning_tools: false,
            auto_update_checks: true,
            telemetry_enabled: false,
        }
    }
}
