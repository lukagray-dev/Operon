//! Models & Providers Settings DTOs.

use serde::{Deserialize, Serialize};

/// Summary of a supported AI provider for the list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSummaryDto {
    pub id: String,
    pub label: String,
    pub status: String,
    pub active_model: String,
    pub is_active: bool,
}

/// Setup form details for a selected provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSetupDetailsDto {
    pub provider_id: String,
    pub provider_label: String,
    pub api_base_url: String,
    pub api_key: String,
    pub active_model: String,
    pub discovered_models: Vec<String>,
}

/// Request payload to persist provider configuration to ~/.operon/config.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveProviderRequestDto {
    pub provider_id: String,
    pub api_base: String,
    pub api_key: String,
    pub selected_model: String,
}
