//! Client abstraction for LLM-backed summarization.
//!
//! The compaction crate does not know how to talk to Anthropic, OpenAI, or any
//! other provider. Runtime code supplies a trait object, while tests can use a
//! deterministic mock.

use crate::CompactionError;

/// Caller-provided summarization client used by [`crate::compact`].
#[async_trait::async_trait]
pub trait CompactionClient: Send + Sync {
    /// Send a summarization prompt to the LLM and return the summary text.
    async fn summarize(&self, prompt: String) -> Result<String, CompactionError>;
}

/// Deterministic client for unit and integration tests.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_client_returns_configured_response() {
        let client = MockCompactionClient {
            response: "summary text".to_string(),
        };

        let result = client.summarize("ignored prompt".to_string()).await;

        assert_eq!(result.ok(), Some("summary text".to_string()));
    }

    #[tokio::test]
    async fn client_trait_is_object_safe() {
        let client = MockCompactionClient {
            response: "object-safe response".to_string(),
        };
        let trait_object: &dyn CompactionClient = &client;

        let result = trait_object.summarize("prompt".to_string()).await;

        assert_eq!(result.ok(), Some("object-safe response".to_string()));
    }
}
