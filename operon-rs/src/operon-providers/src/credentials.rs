// credentials.rs — API credential types for operon-providers.
//
// This module provides `ApiCredentials`, which wraps an API key (and optional
// organization ID) for a specific provider.
//
// SECURITY: The API key is wrapped in `SecretString`, which overrides Debug and
// Display to emit "[REDACTED]" instead of the actual key. This prevents accidental
// key exposure in:
//   - tracing/log output
//   - panic messages
//   - serde Debug output in test failures
//   - any format!() or println!() call that uses {:?} or {}
//
// SERDE: `ApiCredentials` implements Serialize/Deserialize for TOML roundtrip
// via operon-config. SecretString serializes as a plain string (for config files)
// but redacts in Debug/Display.
//
// DO NOT add Clone to SecretString unless explicitly required — limiting copies
// reduces the surface area for accidental exposure.

use serde::{Deserialize, Serialize};
use std::fmt;

// ─────────────────────────────────────────────────────────────────────────────
// SecretString
// ─────────────────────────────────────────────────────────────────────────────

/// A string wrapper that suppresses its value in all debug and display output.
///
/// Used for API keys and other sensitive credentials. The inner value is only
/// accessible via [`SecretString::expose`] — a deliberate, named method call
/// that makes the exposure site visible in code review.
///
/// # Example
///
/// ```rust
/// use operon_providers::credentials::SecretString;
///
/// let key = SecretString::new("sk-abc123".to_string());
/// assert_eq!(format!("{:?}", key), "SecretString([REDACTED])");
/// assert_eq!(format!("{}", key), "[REDACTED]");
/// assert_eq!(key.expose(), "sk-abc123");
/// ```
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)] // serializes/deserializes as a plain string, not a struct
pub struct SecretString(String);

impl SecretString {
    /// Wraps a plain `String` as a secret.
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// Exposes the inner secret value.
    ///
    /// This method is intentionally named `expose` (not `as_str` or `value`)
    /// to make secret access visible and searchable in code review.
    ///
    /// # Usage
    ///
    /// Only call this where the secret is actually needed:
    /// - Building the HTTP `Authorization` or `x-api-key` request header.
    /// - Hashing/comparing credentials.
    ///
    /// Never pass the result to `tracing::info!`, `println!`, or any formatting macro.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Returns true if the key string is empty.
    ///
    /// Useful for validation — an empty API key is almost always a config error.
    /// Ollama is the exception (it runs locally and doesn't require a key).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    // Always prints "[REDACTED]" — never the actual key value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretString([REDACTED])")
    }
}

impl fmt::Display for SecretString {
    // Always prints "[REDACTED]" — never the actual key value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ApiCredentials
// ─────────────────────────────────────────────────────────────────────────────

/// API credentials for a single LLM provider.
///
/// Holds the API key and optional organization ID. Constructed once at startup
/// from `operon-config` and passed into `ProviderConfig`.
///
/// # Provider-specific notes
///
/// - **Anthropic**: `api_key` only. No org ID.
/// - **OpenAI**: `api_key` required; `org_id` optional (used for organization billing).
/// - **Gemini**: `api_key` is the GCP API key. No org ID.
/// - **Ollama**: `api_key` is typically empty — Ollama is local and auth-free.
///   Set to `""` or `"ollama"` (both accepted by the Ollama server).
/// - **OpenRouter**: `api_key` required. No org ID.
/// - All others: `api_key` only.
///
/// `ApiCredentials` is loaded by `operon-config` from:
///   - TOML config file (`~/.operon/config.toml`)
///   - Environment variables (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc.)
///
/// The frontend still constructs it manually when building tests or ad-hoc
/// provider configs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCredentials {
    /// The provider's API key.
    ///
    /// Wrapped in `SecretString` so it never appears in debug output or logs.
    /// Access via `.api_key.expose()` only at the HTTP request construction site.
    pub api_key: SecretString,

    /// Optional organization identifier.
    ///
    /// Currently only used by OpenAI (`OpenAI-Organization` header).
    /// `None` for all other providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
}

impl ApiCredentials {
    /// Creates credentials with an API key and no organization ID.
    ///
    /// This is the correct constructor for all providers except OpenAI
    /// when organization billing is needed.
    pub fn with_key(api_key: impl Into<SecretString>) -> Self {
        Self {
            api_key: api_key.into(),
            org_id: None,
        }
    }

    /// Creates credentials with both an API key and an organization ID.
    ///
    /// Only relevant for OpenAI with organization-level billing separation.
    pub fn with_key_and_org(api_key: impl Into<SecretString>, org_id: String) -> Self {
        Self {
            api_key: api_key.into(),
            org_id: Some(org_id),
        }
    }

    /// Creates empty credentials for providers that don't require authentication.
    ///
    /// Specifically for Ollama, which runs locally and accepts any (or no) API key.
    pub fn unauthenticated() -> Self {
        Self {
            api_key: SecretString::new(String::new()),
            org_id: None,
        }
    }

    /// Returns `true` if the API key is present and non-empty.
    ///
    /// Use for configuration validation at startup. Always returns `true` for
    /// `unauthenticated()` because emptiness is expected for Ollama.
    pub fn has_key(&self) -> bool {
        !self.api_key.is_empty()
    }
}
