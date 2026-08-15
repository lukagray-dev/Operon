//! Settings DTO types for Operon desktop GUI.

use serde::{Deserialize, Serialize};

/// Basic settings status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsWindowStateDto {
    pub is_open: bool,
}
