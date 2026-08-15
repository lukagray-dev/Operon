//! About Settings Data Transfer Objects.

use serde::{Deserialize, Serialize};

/// System and application build specifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AboutSystemInfoDto {
    pub version: String,
    pub platform: String,
    pub architecture: String,
    pub ui_toolkit: String,
    pub compiler: String,
}
