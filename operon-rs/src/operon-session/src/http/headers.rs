// headers.rs — Helper function to construct HTTP request headers for different LLM providers.
//
// Hey friend! This file focuses on a single responsibility: building headers.
// Different providers require different authentication mechanisms. For instance:
//   - Anthropic expects an `x-api-key` header and an API version pin.
//   - OpenAI and other providers expect standard `Authorization: Bearer <key>`.

use reqwest::header::HeaderMap;
use operon_providers::Provider;

/// Build provider-specific request headers from the provider enum + API key.
///
/// Anthropic uses a custom `x-api-key` header plus an API version pin.
/// All other (OpenAI-family) providers use the standard `Authorization: Bearer` header.
pub(crate) fn build_headers(provider: &Provider, api_key: &str) -> HeaderMap {
    use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};

    let mut headers = HeaderMap::new();

    // Every provider requires JSON — set this unconditionally.
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    match provider {
        Provider::Anthropic => {
            // Anthropic uses a custom x-api-key header, not Authorization: Bearer.
            // The unwrap is safe because API keys are ASCII strings.
            headers.insert(
                "x-api-key",
                HeaderValue::from_str(api_key).expect("API key must be a valid header value"),
            );
            // Version pin — ensures we always get the same response shape regardless
            // of future Anthropic API changes.
            headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
        }
        _ => {
            // OpenAI-family and all other providers use Bearer token auth.
            let bearer = format!("Bearer {api_key}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&bearer).expect("API key must be a valid header value"),
            );
        }
    }

    headers
}
