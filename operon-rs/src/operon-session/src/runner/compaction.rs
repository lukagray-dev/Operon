// runner/compaction.rs — Context compaction for SessionRunner.
//
// Contains the private `run_compaction` method that summarizes old history
// when the token budget is exceeded. Self-contained; no further splitting needed.

use operon_context::{compact, AnthropicCompactionClient};
use operon_events::SessionEvent;
use operon_providers::Provider;

use crate::error::SessionError;

use super::SessionRunner;

impl SessionRunner {
    /// Run context compaction: summarize old history and rebuild the message array.
    pub(super) async fn run_compaction(&mut self) -> Result<(), SessionError> {
        let snapshot = self.snapshot_builder.build()?;
        let tokens_before = self.token_state.current_context_tokens;

        match &self.config.provider_config.provider {
            Provider::Anthropic => {
                let compaction_client = AnthropicCompactionClient {
                    api_key: self
                        .config
                        .provider_config
                        .credentials
                        .api_key
                        .expose()
                        .to_string(),
                    model_id: self.config.provider_config.model_id().to_string(),
                    http: self.http_client.clone(),
                };

                let result = compact(
                    self.messages.clone(),
                    &snapshot,
                    &compaction_client,
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
            }
            other => {
                tracing::warn!(
                    "Context compaction not supported for provider {:?} — skipping",
                    other
                );
                let _ = self
                    .event_tx
                    .send(SessionEvent::Warning {
                        message: format!(
                            "Compaction not supported for provider {:?}",
                            self.config.provider_config.provider
                        ),
                    })
                    .await;
            }
        }

        Ok(())
    }
}
