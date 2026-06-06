//! # operon-providers
//!
//! Provider identity, API credentials, model configuration, and capability
//! metadata for Operon's eleven supported LLM providers.
//!
//! ## Role in the architecture
//!
//! This is the **root crate** for all provider-related types. It has zero
//! operon-* dependencies. All four `operon-context-normalize-*` crates
//! re-export [`Provider`] from here rather than defining their own copies,
//! eliminating the four-way enum sync burden that existed before this crate.
//!
//! ```text
//! operon-providers  (this crate — zero operon-* deps)
//!       ↑
//!   ┌───┼───────────────────────────────────────┐
//!   │   │                                       │
//! normalize-tools  normalize-messages  normalize-reasoning  normalize-stream
//!   (re-export Provider from here)
//!       ↑
//! operon-session  (uses ProviderConfig to build HTTP requests)
//!       ↑
//! operon-config   (loads ProviderConfig from TOML/env via AppConfig)
//! ```
//!
//! ## What this crate provides
//!
//! | Type | Purpose |
//! |---|---|
//! | [`Provider`] | 11-variant enum identifying the LLM provider |
//! | [`ProviderCapabilities`] | Per-provider metadata (base URL, auth style, feature flags) |
//! | [`AuthHeader`] | How to transmit the API key in HTTP headers |
//! | [`ApiCredentials`] | API key (redacted in logs) + optional org ID |
//! | [`ModelConfig`] | Model ID string + context window + max_tokens |
//! | [`ProviderConfig`] | Assembled runtime config: provider + credentials + model + URL |
//!
//! ## Supported providers
//!
//! | Variant | Company | Wire family |
//! |---|---|---|
//! | [`Provider::Anthropic`] | Anthropic | Anthropic Messages API |
//! | [`Provider::OpenAI`] | OpenAI | Chat Completions API |
//! | [`Provider::Gemini`] | Google | GenerateContent API |
//! | [`Provider::Ollama`] | Ollama | OpenAI-compatible (local) |
//! | [`Provider::DeepSeek`] | DeepSeek | OpenAI-compatible |
//! | [`Provider::OpenRouter`] | OpenRouter | Auto-detect |
//! | [`Provider::Groq`] | Groq | OpenAI-compatible |
//! | [`Provider::Mistral`] | Mistral | OpenAI-compatible |
//! | [`Provider::XAI`] | xAI | OpenAI-compatible |
//! | [`Provider::NvidiaNim`] | NVIDIA | OpenAI-compatible |
//! | [`Provider::Cohere`] | Cohere | Cohere Chat API |
//!
//! ## Quick start
//!
//! ```rust
//! use operon_providers::{Provider, ProviderConfig};
//! use operon_providers::credentials::ApiCredentials;
//! use operon_providers::model::ModelConfig;
//!
//! // Build a session config for Anthropic Claude Sonnet 4
//! let config = ProviderConfig {
//!     provider:          Provider::Anthropic,
//!     credentials:       ApiCredentials::with_key("sk-ant-..."),
//!     model: ModelConfig {
//!         model_id: "claude-3-5-sonnet-latest".to_string(),
//!         context_window: 200_000,
//!         max_tokens: 8_192,
//!     },
//!     base_url_override: None,
//! };
//!
//! assert_eq!(config.effective_base_url(), "https://api.anthropic.com/v1");
//! assert_eq!(config.model_id(), "claude-3-5-sonnet-latest");
//! ```

pub mod config;
pub mod credentials;
pub mod discovery;
pub mod model;
pub mod provider;

// ─────────────────────────────────────────────────────────────────────────────
// Public re-exports
// ─────────────────────────────────────────────────────────────────────────────

// The core enum — re-exported at crate root for convenience.
// Normalize crates import `operon_providers::Provider` directly.
pub use provider::{AuthHeader, Provider, ProviderCapabilities};

// Credential types.
pub use credentials::{ApiCredentials, SecretString};

// Model configuration + presets.
pub use model::ModelConfig;

// Assembled runtime config — what the session runner consumes.
pub use config::ProviderConfig;

// Model discovery.
pub use discovery::{discover_models, DiscoveredModel, DiscoveryResult};
