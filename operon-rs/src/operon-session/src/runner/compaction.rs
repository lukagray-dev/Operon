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
        let base_url = self.config.provider_config.effective_base_url();
        let endpoint = match &self.config.provider_config.provider {
            Provider::Anthropic => format!("{}/messages", base_url.trim_end_matches('/')),
            Provider::Gemini => {
                let clean_id = model_id.strip_prefix("models/").unwrap_or(&model_id);
                format!(
                    "{}/models/{}:generateContent",
                    base_url.trim_end_matches('/'),
                    clean_id
                )
            }
            Provider::Cohere => format!("{}/chat", base_url.trim_end_matches('/')),
            _ => format!("{}/chat/completions", base_url.trim_end_matches('/')),
        };

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

        // Ensure compaction config uses the real model context window from provider_config
        let mut compaction_cfg = self.config.compaction.clone();
        compaction_cfg.context_window = self.config.provider_config.context_window();

        let result = compact(
            self.messages.clone(),
            &snapshot,
            client.as_ref(),
            &compaction_cfg,
            tokens_before,
        )
        .await?;

        self.messages = result.messages;
        self.token_state.reset();
        if result.tokens_after > 0 {
            self.token_state
                .apply_estimate(result.tokens_after, operon_context::EstimationTier::Exact);
        }
        self.dispatcher.notify_compaction();

        // Persist the compacted history baseline to disk so subsequent turns and session restarts
        // continue from the compacted snapshot + summary rather than reloading uncompacted history.
        if let Some(store) = &self.store {
            let baseline_len = self.messages.len().saturating_sub(1);
            let compacted_baseline = &self.messages[..baseline_len];
            let _ = store
                .apply_compaction(
                    &self.session_id,
                    compacted_baseline,
                    Some(result.tokens_after),
                )
                .await;
        }

        // Reset turn index to 1 (turn 0 is the compacted baseline, current turn will be turn 1)
        self.turn_index = 1;

        let _ = self
            .event_tx
            .send(SessionEvent::CompactionOccurred {
                tokens_before,
                tokens_after: result.tokens_after,
                summary: result.summary,
            })
            .await;

        self.emit_context_usage_update().await;

        Ok(())
    }
}
