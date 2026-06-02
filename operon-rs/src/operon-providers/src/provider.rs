// provider.rs — The canonical Provider enum and per-provider capability metadata.
//
// This is the single authoritative definition of Provider in the entire Operon codebase.
// All four operon-context-normalize-* crates re-export this enum rather than defining
// their own copies, eliminating the sync burden that existed before this crate.
//
// DESIGN: Provider is a pure tag enum — it carries no data. All provider-specific
// behavior (wire format dispatch, base URLs, auth headers) is derived from it via
// `capabilities()` or matched on in the normalize crates. The enum itself is kept
// small and stable so adding it to configs and persisted records is low-cost.
//
// ADDING A NEW PROVIDER:
//   1. Add a variant here.
//   2. Add a `capabilities()` arm below.
//   3. Add the matching provider module in each of the four normalize crates.
//   That's the complete checklist — no other files in this crate need changing.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Provider
// ─────────────────────────────────────────────────────────────────────────────

/// Identifies which LLM provider to use for a session.
///
/// This enum is the single source of truth for provider identity across the
/// entire Operon codebase. The four `operon-context-normalize-*` crates all
/// re-export this type rather than defining their own copies.
///
/// # Serde representation
///
/// Serializes as a lowercase snake_case string:
/// `Anthropic` → `"anthropic"`, `OpenRouter` → `"open_router"`, etc.
/// This is the key name used in `operon-config` TOML files.
///
/// # Equality and hashing
///
/// `Provider` is `Eq + Hash` so it can be used as a `HashMap` key
/// (e.g., in per-provider credential maps in `operon-config`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// Anthropic Claude models (claude-opus-4, claude-sonnet-4, etc.)
    /// Wire format: Anthropic Messages API. Auth: `x-api-key` header.
    Anthropic,

    /// OpenAI GPT models (gpt-4o, gpt-4-turbo, o3, o4-mini, etc.)
    /// Wire format: OpenAI Chat Completions API. Auth: `Authorization: Bearer`.
    #[serde(rename = "open_ai")]
    OpenAI,

    /// Google Gemini models (gemini-2.5-pro, gemini-flash, etc.)
    /// Wire format: Google GenerateContent API. Auth: `x-goog-api-key` header.
    Gemini,

    /// Ollama local model server — runs models locally, no API key required.
    /// Wire format: OpenAI-compatible `/v1/` endpoints.
    /// Default base URL: `http://localhost:11434`.
    Ollama,

    /// DeepSeek models — OpenAI-compatible wire format with `reasoning_content`.
    /// Auth: `Authorization: Bearer`.
    DeepSeek,

    /// OpenRouter gateway — proxies to many providers; auto-detects wire shape.
    /// Auth: `Authorization: Bearer`. Requires `HTTP-Referer` and `X-Title` headers.
    OpenRouter,

    /// Groq inference API — OpenAI-compatible, extremely fast inference.
    /// Auth: `Authorization: Bearer`.
    Groq,

    /// Mistral AI models — OpenAI-compatible wire format.
    /// Auth: `Authorization: Bearer`.
    Mistral,

    /// xAI Grok models (grok-4, grok-4-vision, etc.) — OpenAI-compatible wire format
    /// with `reasoning_content` field for reasoning models.
    /// Auth: `Authorization: Bearer`.
    #[serde(rename = "xai")]
    XAI,

    /// Cohere Command models — distinct wire format (`parameter_definitions` instead
    /// of JSON Schema). Auth: `Authorization: Bearer`.
    Cohere,
}

impl Provider {
    /// Returns the capability metadata for this provider.
    ///
    /// Use this to determine base URLs, auth header styles, and feature support
    /// without hardcoding provider-specific strings throughout the session layer.
    pub fn capabilities(self) -> ProviderCapabilities {
        match self {
            Provider::Anthropic => ProviderCapabilities {
                default_base_url: "https://api.anthropic.com/v1",
                auth_header: AuthHeader::XApiKey,
                supports_streaming: true,
                // Extended thinking is a request-time opt-in for claude-3-7+ models.
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::OpenAI => ProviderCapabilities {
                default_base_url: "https://api.openai.com/v1",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // o1/o3/o4 models expose reasoning_summary — not all GPT-4o models.
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::Gemini => ProviderCapabilities {
                default_base_url: "https://generativelanguage.googleapis.com/v1beta",
                auth_header: AuthHeader::XGoogApiKey,
                supports_streaming: true,
                // Gemini 2.5+ supports thinking via thought parts.
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::Ollama => ProviderCapabilities {
                // Ollama runs locally — operator may override this via base_url_override.
                default_base_url: "http://localhost:11434/v1",
                // Ollama doesn't require an API key — empty string is the convention.
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // Some models served by Ollama support thinking (e.g. qwq, deepseek-r1).
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::DeepSeek => ProviderCapabilities {
                default_base_url: "https://api.deepseek.com/v1",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // deepseek-reasoner exposes reasoning_content.
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::OpenRouter => ProviderCapabilities {
                default_base_url: "https://openrouter.ai/api/v1",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // Depends on underlying model — we conservatively say true.
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::Groq => ProviderCapabilities {
                default_base_url: "https://api.groq.com/openai/v1",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // Groq does not expose chain-of-thought reasoning content.
                supports_thinking: false,
                supports_tool_use: true,
            },
            Provider::Mistral => ProviderCapabilities {
                default_base_url: "https://api.mistral.ai/v1",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // Mistral does not expose reasoning content currently.
                supports_thinking: false,
                supports_tool_use: true,
            },
            Provider::XAI => ProviderCapabilities {
                default_base_url: "https://api.x.ai/v1",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // Grok reasoning models expose reasoning_content.
                supports_thinking: true,
                supports_tool_use: true,
            },
            Provider::Cohere => ProviderCapabilities {
                default_base_url: "https://api.cohere.com/v2",
                auth_header: AuthHeader::Bearer,
                supports_streaming: true,
                // Cohere does not expose chain-of-thought reasoning content.
                supports_thinking: false,
                supports_tool_use: true,
            },
        }
    }

    /// Returns the display name of this provider (for UI and logs).
    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic",
            Provider::OpenAI => "OpenAI",
            Provider::Gemini => "Google Gemini",
            Provider::Ollama => "Ollama",
            Provider::DeepSeek => "DeepSeek",
            Provider::OpenRouter => "OpenRouter",
            Provider::Groq => "Groq",
            Provider::Mistral => "Mistral",
            Provider::XAI => "xAI",
            Provider::Cohere => "Cohere",
        }
    }

    /// Returns all supported providers as a static slice.
    ///
    /// Useful for UI dropdowns, config validation, and documentation generation.
    pub fn all() -> &'static [Provider] {
        &[
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Gemini,
            Provider::Ollama,
            Provider::DeepSeek,
            Provider::OpenRouter,
            Provider::Groq,
            Provider::Mistral,
            Provider::XAI,
            Provider::Cohere,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AuthHeader
// ─────────────────────────────────────────────────────────────────────────────

/// Specifies how the API key is transmitted in the HTTP request header.
///
/// Used by the session runner's HTTP layer (`operon-session/src/http.rs`) to
/// build the correct `Authorization` or custom header for each provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthHeader {
    /// `Authorization: Bearer <api_key>` — used by OpenAI, Groq, Mistral, etc.
    Bearer,
    /// `x-api-key: <api_key>` — used by Anthropic.
    XApiKey,
    /// `x-goog-api-key: <api_key>` — used by Google Gemini.
    XGoogApiKey,
}

// ─────────────────────────────────────────────────────────────────────────────
// ProviderCapabilities
// ─────────────────────────────────────────────────────────────────────────────

/// Static capability metadata for a provider.
///
/// Returned by [`Provider::capabilities()`]. Consumed by the session HTTP layer
/// to construct requests correctly without hardcoded per-provider branches.
///
/// # Notes on `supports_thinking`
///
/// This flag means the provider's wire format CAN carry reasoning/thinking content
/// for at least some models. It does NOT mean every model from that provider
/// supports thinking — that is a per-model property. The session runner uses this
/// flag to decide whether to attempt parsing `ReasoningBlock`s from the stream.
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    /// The canonical base URL for this provider's API.
    ///
    /// The session runner appends the specific path (e.g. `/chat/completions`).
    /// This is the default — `ProviderConfig::effective_base_url()` returns the
    /// `base_url_override` if set (for self-hosted deployments or proxies).
    pub default_base_url: &'static str,

    /// How the API key is sent in HTTP request headers.
    pub auth_header: AuthHeader,

    /// Whether this provider supports server-sent event (SSE) streaming.
    ///
    /// Currently true for all ten providers. Kept explicit for future-proofing.
    pub supports_streaming: bool,

    /// Whether this provider's wire format can carry reasoning/thinking content
    /// for at least some models.
    pub supports_thinking: bool,

    /// Whether this provider supports tool/function calling in the standard sense.
    ///
    /// Currently true for all ten providers.
    pub supports_tool_use: bool,
}
