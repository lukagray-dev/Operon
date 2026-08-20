// config.rs — The assembled ProviderConfig for operon-providers.
//
// `ProviderConfig` is the single struct the session runner needs to:
//   1. Know which provider to talk to (Provider enum)
//   2. Know which model to request (ModelConfig)
//   3. Authenticate the HTTP request (ApiCredentials)
//   4. Determine the correct API endpoint (default or overridden base URL)
//
// It replaces the three scattered fields in SessionConfig
// (provider: Provider, api_key: String, model_id: String) with a single
// cohesive type that is easy to pass around, validate, and eventually
// serialize to/from TOML via operon-config.
//
// ProviderConfig is loaded by operon-config from:
//   - TOML config file: ~/.operon/config.toml
//   - Environment variables: ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.
// The frontend still constructs it manually when building a SessionConfig.

use serde::{Deserialize, Serialize};

use crate::credentials::ApiCredentials;
use crate::model::ModelConfig;
use crate::provider::{AuthHeader, Provider};

// ─────────────────────────────────────────────────────────────────────────────
// ProviderConfig
// ─────────────────────────────────────────────────────────────────────────────

/// Complete runtime configuration for a single LLM provider connection.
///
/// This is the type passed to `SessionRunner::new()` (once the session crate
/// is updated). It replaces the ad-hoc `provider`, `api_key`, and `model_id`
/// fields that were previously scattered across `SessionConfig`.
///
/// # Example
///
/// ```rust
/// use operon_providers::{Provider, ProviderConfig};
/// use operon_providers::credentials::ApiCredentials;
/// use operon_providers::model::ModelConfig;
///
/// let config = ProviderConfig {
///     provider:          Provider::Anthropic,
///     credentials:       ApiCredentials::with_key("sk-ant-..."),
///     model: ModelConfig {
///         model_id: "claude-3-5-sonnet-latest".to_string(),
///         context_window: 200_000,
///         max_tokens: 8_192,
///         reasoning_effort: None,
///     },
///     base_url_override: None,
/// };
///
/// assert_eq!(config.effective_base_url(), "https://api.anthropic.com/v1");
/// assert!(config.credentials.has_key());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Which LLM provider to use for this session.
    pub provider: Provider,

    /// API credentials for this provider.
    ///
    /// For Ollama (local), use `ApiCredentials::unauthenticated()`.
    pub credentials: ApiCredentials,

    /// The model to request from this provider.
    pub model: ModelConfig,

    /// Optional override for the provider's API base URL.
    ///
    /// Set this when using:
    /// - A self-hosted Ollama instance at a non-default address
    /// - A corporate proxy in front of a provider
    /// - A local LM Studio server (OpenAI-compatible)
    /// - Any deployment where the default base URL is wrong
    ///
    /// Example: `Some("http://10.0.0.5:11434/v1".to_string())` for a remote Ollama.
    ///
    /// When `None`, `effective_base_url()` returns the provider's canonical default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url_override: Option<String>,
}

impl ProviderConfig {
    /// Returns the API base URL to use for HTTP requests.
    ///
    /// Returns `base_url_override` if set, otherwise the provider's canonical
    /// default URL from `Provider::capabilities().default_base_url`.
    ///
    /// The session HTTP layer appends the specific path after this base URL,
    /// e.g. `/chat/completions` or `/messages`.
    pub fn effective_base_url(&self) -> &str {
        match &self.base_url_override {
            Some(url) => url.as_str(),
            None => self.provider.capabilities().default_base_url,
        }
    }

    /// Returns the auth header style this provider uses.
    ///
    /// Convenience wrapper around `Provider::capabilities().auth_header`.
    /// Used by the session HTTP layer to build the correct request header.
    pub fn auth_header(&self) -> AuthHeader {
        self.provider.capabilities().auth_header
    }

    /// Returns true if this config has a non-empty API key.
    ///
    /// Use for config validation at startup. Always true for unauthenticated
    /// providers (Ollama) since they don't need a key.
    pub fn has_credentials(&self) -> bool {
        // For Ollama, an empty key is expected and valid — return true anyway.
        if self.provider == Provider::Ollama {
            return true;
        }
        self.credentials.has_key()
    }

    /// Returns the model identifier string to include in the request body.
    ///
    /// Convenience accessor: `config.model_id()` instead of `config.model.model_id`.
    pub fn model_id(&self) -> &str {
        &self.model.model_id
    }

    /// Returns the context window size for this model.
    ///
    /// Passed to `TokenBudget` in the session runner.
    pub fn context_window(&self) -> usize {
        self.model.context_window
    }

    /// Returns the max_tokens value to send per turn.
    pub fn max_tokens(&self) -> usize {
        self.model.max_tokens
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn anthropic_config() -> ProviderConfig {
        ProviderConfig {
            provider: Provider::Anthropic,
            credentials: ApiCredentials::with_key("sk-ant-test-key"),
            model: ModelConfig {
                model_id: "claude-sonnet-4-20250514".to_string(),
                context_window: 200_000,
                max_tokens: 16_000,
                reasoning_effort: None,
            },
            base_url_override: None,
        }
    }

    fn ollama_config() -> ProviderConfig {
        ProviderConfig {
            provider: Provider::Ollama,
            credentials: ApiCredentials::unauthenticated(),
            model: ModelConfig {
                model_id: "llama3.2".to_string(),
                context_window: 128_000,
                max_tokens: 8_192,
                reasoning_effort: None,
            },
            base_url_override: None,
        }
    }

    #[test]
    fn test_effective_base_url_uses_default_when_no_override() {
        let config = anthropic_config();
        assert_eq!(config.effective_base_url(), "https://api.anthropic.com/v1");
    }

    #[test]
    fn test_effective_base_url_uses_override_when_set() {
        let mut config = anthropic_config();
        config.base_url_override = Some("https://proxy.internal/anthropic/v1".to_string());
        assert_eq!(
            config.effective_base_url(),
            "https://proxy.internal/anthropic/v1"
        );
    }

    #[test]
    fn test_ollama_default_base_url() {
        let config = ollama_config();
        assert_eq!(config.effective_base_url(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_ollama_custom_base_url() {
        let mut config = ollama_config();
        config.base_url_override = Some("http://10.0.0.5:11434/v1".to_string());
        assert_eq!(config.effective_base_url(), "http://10.0.0.5:11434/v1");
    }

    #[test]
    fn test_auth_header_anthropic() {
        let config = anthropic_config();
        assert_eq!(config.auth_header(), AuthHeader::XApiKey);
    }

    #[test]
    fn test_auth_header_openai() {
        let config = ProviderConfig {
            provider: Provider::OpenAI,
            credentials: ApiCredentials::with_key("sk-test"),
            model: ModelConfig {
                model_id: "gpt-4o".to_string(),
                context_window: 128_000,
                max_tokens: 16_384,
                reasoning_effort: None,
            },
            base_url_override: None,
        };
        assert_eq!(config.auth_header(), AuthHeader::Bearer);
    }

    #[test]
    fn test_has_credentials_true_for_key_bearing_provider() {
        let config = anthropic_config();
        assert!(
            config.has_credentials(),
            "Anthropic with key should have credentials"
        );
    }

    #[test]
    fn test_has_credentials_true_for_ollama_even_without_key() {
        let config = ollama_config();
        // Ollama doesn't need a key — has_credentials() is always true for it.
        assert!(
            config.has_credentials(),
            "Ollama should pass credentials check even without key"
        );
    }

    #[test]
    fn test_has_credentials_false_for_empty_key() {
        let config = ProviderConfig {
            provider: Provider::OpenAI,
            credentials: ApiCredentials::with_key(""),
            model: ModelConfig {
                model_id: "gpt-4o".to_string(),
                context_window: 128_000,
                max_tokens: 16_384,
                reasoning_effort: None,
            },
            base_url_override: None,
        };
        assert!(
            !config.has_credentials(),
            "Empty key should fail credentials check"
        );
    }

    #[test]
    fn test_model_accessors() {
        let config = anthropic_config();
        assert_eq!(config.model_id(), "claude-sonnet-4-20250514");
        assert_eq!(config.context_window(), 200_000);
        assert_eq!(config.max_tokens(), 16_000);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = anthropic_config();
        let json = serde_json::to_string(&original).unwrap();
        let restored: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(original.model_id(), restored.model_id());
        assert_eq!(original.provider, restored.provider);
        assert_eq!(original.context_window(), restored.context_window());
    }
}
