//! Client abstraction for LLM-backed summarization.
//!
//! The compaction crate does not know how to talk to Anthropic, OpenAI, or any
//! other provider. Runtime code supplies a trait object, while tests can use a
//! deterministic mock.
//!
//! ## Feature flags
//!
//! | Feature        | What it adds                                        |
//! |----------------|-----------------------------------------------------|
//! | `http-client`  | [`AnthropicCompactionClient`] (real HTTP via reqwest)|
//! | `test-utils`   | [`MockCompactionClient`] available outside tests    |

use crate::CompactionError;

// ─────────────────────────────────────────────────────────────────────────────
// CompactionClient trait
// ─────────────────────────────────────────────────────────────────────────────

/// Caller-provided summarization client used by [`crate::compact`].
///
/// Implement this trait to plug in any LLM provider. The compaction crate
/// itself is provider-agnostic — it only calls `summarize` with the assembled
/// prompt and expects the summary text back.
#[async_trait::async_trait]
pub trait CompactionClient: Send + Sync {
    /// Send a summarization prompt to the LLM and return the summary text.
    async fn summarize(&self, prompt: String) -> Result<String, CompactionError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// MockCompactionClient — test-only deterministic stub
// ─────────────────────────────────────────────────────────────────────────────

/// Deterministic client for unit and integration tests.
///
/// Every call to `summarize` returns the pre-configured `response` string,
/// regardless of the prompt. This makes compaction tests hermetic and fast.
#[cfg(any(test, feature = "test-utils"))]
#[derive(Debug, Clone)]
pub struct MockCompactionClient {
    /// Response returned from every `summarize` call.
    pub response: String,
}

#[cfg(any(test, feature = "test-utils"))]
#[async_trait::async_trait]
impl CompactionClient for MockCompactionClient {
    async fn summarize(&self, _prompt: String) -> Result<String, CompactionError> {
        Ok(self.response.clone())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AnthropicCompactionClient — real HTTP implementation (feature = "http-client")
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete Anthropic HTTP client for context compaction summarization.
///
/// Sends a single non-streaming POST to `https://api.anthropic.com/v1/messages`
/// using the supplied API key and model. The response's `content[0].text` field
/// is extracted and returned as the summary string.
///
/// # Feature gate
///
/// Only compiled when the `http-client` feature is enabled. This keeps the
/// `operon-context-compaction` crate lightweight — tests and other consumers
/// that only need the trait and mock stay free of the `reqwest` dependency.
///
/// # Example
///
/// ```rust,ignore
/// use operon_context_compaction::AnthropicCompactionClient;
///
/// let client = AnthropicCompactionClient {
///     api_key: std::env::var("ANTHROPIC_API_KEY").unwrap(),
///     model_id: "claude-sonnet-4-20250514".to_string(),
///     http: reqwest::Client::new(),
/// };
/// ```
#[cfg(feature = "http-client")]
pub struct AnthropicCompactionClient {
    /// Anthropic API key — passed verbatim as the `x-api-key` request header.
    pub api_key: String,
    /// Model identifier, e.g. `"claude-sonnet-4-20250514"`.
    pub model_id: String,
    /// Shared HTTP client. Clone from an existing `reqwest::Client` to reuse
    /// connection pools and TLS sessions across the session lifecycle.
    pub http: reqwest::Client,
}

#[cfg(feature = "http-client")]
#[async_trait::async_trait]
impl CompactionClient for AnthropicCompactionClient {
    /// Summarize `prompt` via a single-shot (non-streaming) Anthropic API call.
    ///
    /// ## Request shape
    ///
    /// ```json
    /// { "model": "<model_id>", "max_tokens": 8096, "messages": [{ "role": "user", "content": "<prompt>" }] }
    /// ```
    ///
    /// ## Response extraction
    ///
    /// Anthropic returns `{ "content": [{ "type": "text", "text": "..." }], ... }`.
    /// This implementation extracts `content[0].text` and returns it.
    ///
    /// # Errors
    ///
    /// - [`CompactionError::ClientError`] on any HTTP or network failure.
    /// - [`CompactionError::ClientError`] if the response body cannot be parsed
    ///   or `content[0].text` is absent.
    async fn summarize(&self, prompt: String) -> Result<String, CompactionError> {
        // Build the request body — single user message containing the full prompt.
        // max_tokens=8096 is generous enough for a dense summarization response
        // without wasting quota on a simple summary call.
        let body = serde_json::json!({
            "model": self.model_id,
            "max_tokens": 8096,
            "messages": [
                { "role": "user", "content": prompt }
            ]
        });

        // Send the POST request to the Anthropic Messages API.
        // No streaming — we want the full response in one payload.
        let response = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            // Anthropic requires the API key in this custom header, not Bearer.
            .header("x-api-key", &self.api_key)
            // Version pinning ensures consistent response shape regardless of
            // future breaking Anthropic API changes.
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| CompactionError::ClientError(e.to_string()))?;

        // Non-2xx status → extract body text for a helpful error message.
        // We map this to ClientError so the compaction pipeline can surface
        // the reason without knowing anything about HTTP internals.
        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(CompactionError::ClientError(format!(
                "Anthropic API returned HTTP {status}: {text}"
            )));
        }

        // Deserialize the full JSON response body.
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CompactionError::ClientError(format!("Failed to parse JSON: {e}")))?;

        // Navigate to content[0].text — Anthropic always provides at least one
        // text content block on a successful non-streaming call.
        let text = json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                CompactionError::ClientError(
                    "Anthropic response missing content[0].text".to_string(),
                )
            })?
            .to_string();

        Ok(text)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAICompactionClient — real HTTP implementation (feature = "http-client")
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete OpenAI-compatible HTTP client for context compaction summarization.
///
/// Supports OpenAI, NVIDIA NIM, OpenRouter, Groq, DeepSeek, Mistral, xAI, Ollama, and Cohere.
#[cfg(feature = "http-client")]
pub struct OpenAICompactionClient {
    /// API key — passed in `Authorization: Bearer <key>` header (optional for local Ollama).
    pub api_key: String,
    /// Model identifier.
    pub model_id: String,
    /// API endpoint URL.
    pub endpoint: String,
    /// Shared HTTP client.
    pub http: reqwest::Client,
}

#[cfg(feature = "http-client")]
#[async_trait::async_trait]
impl CompactionClient for OpenAICompactionClient {
    async fn summarize(&self, prompt: String) -> Result<String, CompactionError> {
        let body = serde_json::json!({
            "model": self.model_id,
            "messages": [
                { "role": "user", "content": prompt }
            ]
        });

        let mut req = self
            .http
            .post(&self.endpoint)
            .header("content-type", "application/json");

        if !self.api_key.is_empty() {
            req = req.header("authorization", format!("Bearer {}", self.api_key));
        }

        let response = req
            .json(&body)
            .send()
            .await
            .map_err(|e| CompactionError::ClientError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(CompactionError::ClientError(format!(
                "OpenAI API returned HTTP {status}: {text}"
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CompactionError::ClientError(format!("Failed to parse JSON: {e}")))?;

        let text = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                CompactionError::ClientError(
                    "OpenAI response missing choices[0].message.content".to_string(),
                )
            })?
            .to_string();

        Ok(text)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GeminiCompactionClient — real HTTP implementation (feature = "http-client")
// ─────────────────────────────────────────────────────────────────────────────

/// Concrete Google Gemini HTTP client for context compaction summarization.
#[cfg(feature = "http-client")]
pub struct GeminiCompactionClient {
    /// Google API key — passed in `x-goog-api-key` header.
    pub api_key: String,
    /// Model identifier.
    pub model_id: String,
    /// API endpoint URL.
    pub endpoint: String,
    /// Shared HTTP client.
    pub http: reqwest::Client,
}

#[cfg(feature = "http-client")]
#[async_trait::async_trait]
impl CompactionClient for GeminiCompactionClient {
    async fn summarize(&self, prompt: String) -> Result<String, CompactionError> {
        let body = serde_json::json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": prompt }]
                }
            ]
        });

        let mut req = self
            .http
            .post(&self.endpoint)
            .header("content-type", "application/json");

        if !self.api_key.is_empty() {
            req = req.header("x-goog-api-key", &self.api_key);
        }

        let response = req
            .json(&body)
            .send()
            .await
            .map_err(|e| CompactionError::ClientError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable body>".to_string());
            return Err(CompactionError::ClientError(format!(
                "Gemini API returned HTTP {status}: {text}"
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CompactionError::ClientError(format!("Failed to parse JSON: {e}")))?;

        let text = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|arr| arr.first())
            .and_then(|part| part.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                CompactionError::ClientError(
                    "Gemini response missing candidates[0].content.parts[0].text".to_string(),
                )
            })?
            .to_string();

        Ok(text)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_client_returns_configured_response() {
        // The mock should return its configured response regardless of the prompt.
        let client = MockCompactionClient {
            response: "summary text".to_string(),
        };

        let result = client.summarize("ignored prompt".to_string()).await;

        assert_eq!(result.ok(), Some("summary text".to_string()));
    }

    #[tokio::test]
    async fn client_trait_is_object_safe() {
        // CompactionClient must be object-safe so callers can use Box<dyn CompactionClient>.
        let client = MockCompactionClient {
            response: "object-safe response".to_string(),
        };
        let trait_object: &dyn CompactionClient = &client;

        let result = trait_object.summarize("prompt".to_string()).await;

        assert_eq!(result.ok(), Some("object-safe response".to_string()));
    }
}
