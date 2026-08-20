// runner/compaction.rs — Context compaction for SessionRunner.
//
// Contains the private `run_compaction` method that summarizes old history
// when the token budget is exceeded. Self-contained; no further splitting needed.

use operon_context::{
    compact, AnthropicCompactionClient, CompactionClient, GeminiCompactionClient,
    OpenAICompactionClient,
};
use operon_events::SessionEvent;
use operon_providers::Provider;

use crate::error::SessionError;

use super::SessionRunner;

impl SessionRunner {
    /// Run context compaction: summarize old history and rebuild the message array.
    pub(super) async fn run_compaction(&mut self) -> Result<(), SessionError> {
        let snapshot = self.snapshot_builder.build()?;
        let tokens_before = self.token_state.current_context_tokens;

        let api_key = self
            .config
            .provider_config
            .credentials
            .api_key
            .expose()
            .to_string();
        let model_id = self.config.provider_config.model_id().to_string();
        let endpoint = self.config.provider_config.effective_base_url().to_string();

        let client: Box<dyn CompactionClient> = match &self.config.provider_config.provider {
            Provider::Anthropic => Box::new(AnthropicCompactionClient {
                api_key,
                model_id,
                http: self.http_client.clone(),
            }),
            Provider::Gemini => Box::new(GeminiCompactionClient {
                api_key,
                model_id,
                endpoint,
                http: self.http_client.clone(),
            }),
            // OpenAI, NVIDIA NIM, OpenRouter, Groq, DeepSeek, Mistral, xAI, Ollama, Cohere:
            _ => Box::new(OpenAICompactionClient {
                api_key,
                model_id,
                endpoint,
                http: self.http_client.clone(),
            }),
        };

        let result = compact(
            self.messages.clone(),
            &snapshot,
            client.as_ref(),
            &self.config.compaction,
            tokens_before,
        )
        .await?;

        self.messages = result.messages;
        self.token_state.reset();
        self.dispatcher.notify_compaction();

        let _ = self
            .event_tx
            .send(SessionEvent::CompactionOccurred {
                tokens_before,
                tokens_after: result.tokens_after,
            })
            .await;

        self.emit_context_usage_update().await;

        Ok(())
    }
}
