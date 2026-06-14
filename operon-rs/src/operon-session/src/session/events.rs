// events.rs — Formats and structures session events, token usage records, and assistant messages.
//
// Hey friend! This file does all the heavy lifting for formatting event payloads,
// tracking tokens, and constructing the final assistant message blocks.

use std::sync::Arc;
use serde_json::Value;

use crate::runner::SessionRunner;
use crate::http::StreamResult;
use operon_context::{
    ContentBlock, ConversationMessage, TokenBudget, ToolContent, ToolResult, UsageRecord,
};
use operon_events::SessionEvent;
use operon_tools::ToolProgressEmitter;

impl SessionRunner {
    /// Emit the current context-window gauge for the UI.
    pub(crate) async fn emit_context_usage_update(&self) {
        let _ = self
            .event_tx
            .send(context_usage_event(
                &self.token_budget,
                self.token_state.current_context_tokens,
            ))
            .await;
    }

    /// Build a synchronous progress callback that forwards tool progress into the event bus.
    ///
    /// The callback uses `try_send` so tool code can report progress without
    /// blocking on the async runtime.
    #[allow(dead_code)]
    pub(crate) fn tool_progress_emitter(&self) -> ToolProgressEmitter {
        let event_tx = self.event_tx.clone();

        Arc::new(move |progress| {
            let _ = event_tx.try_send(SessionEvent::ToolProgress(progress));
        })
    }
}

/// Convert a tool result into the serialized content string emitted on the event bus.
pub fn tool_result_content_json(result: &ToolResult) -> String {
    match &result.content {
        ToolContent::Text(text) => text.clone(),
    }
}

/// Build the context gauge event from the current token state.
pub fn context_usage_event(token_budget: &TokenBudget, current_context_tokens: usize) -> SessionEvent {
    let context_window = token_budget.context_window();
    let remaining_context_tokens = context_window.saturating_sub(current_context_tokens);

    SessionEvent::ContextUsageUpdated {
        current_context_tokens,
        context_window,
        remaining_context_tokens,
        utilization: token_budget.utilization(current_context_tokens),
        compaction_limit: token_budget.compaction_limit(),
    }
}

/// Build a `ConversationMessage` from a fully assembled `StreamResult`.
pub fn build_assistant_message(result: &StreamResult) -> ConversationMessage {
    let mut blocks: Vec<ContentBlock> = Vec::new();

    // Hey friend! If the model did some reasoning/thinking during this turn, we prepend it as the
    // very first block in the message. This ensures the thinking block resides before the text or tool
    // blocks, which matches the model's actual execution flow and keeps providers like Anthropic happy!
    if let Some(reasoning) = &result.reasoning {
        blocks.push(ContentBlock::Reasoning(reasoning.clone()));
    }

    if !result.text.is_empty() {
        blocks.push(ContentBlock::Text(result.text.clone()));
    }

    for call in &result.tool_calls {
        blocks.push(ContentBlock::ToolCall(call.clone()));
    }

    let mut msg = ConversationMessage::assistant(blocks);

    if let Some(stop) = &result.stop_reason {
        msg = msg.with_stop(stop.clone());
    }

    msg
}

/// Extract a `UsageRecord` from a raw usage metadata JSON value.
///
/// Handles both Anthropic and OpenAI usage shapes:
///   - Anthropic: `{ "input_tokens": N, "output_tokens": N, "cache_read_input_tokens": N, ... }`
///   - OpenAI:    `{ "prompt_tokens": N, "completion_tokens": N }`
///
/// Returns `None` if the required fields are absent.
pub fn extract_usage_record(
    raw: &Value,
    model_id: &str,
    provider_name: &str,
) -> Option<UsageRecord> {
    let input = raw
        .get("input_tokens")
        .or_else(|| raw.get("prompt_tokens"))
        .and_then(|v| v.as_u64())? as usize;

    let output = raw
        .get("output_tokens")
        .or_else(|| raw.get("completion_tokens"))
        .and_then(|v| v.as_u64())? as usize;

    let cache_read = raw
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    let cache_write = raw
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize);

    Some(UsageRecord {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: cache_read,
        cache_write_tokens: cache_write,
        model: model_id.to_string(),
        provider: provider_name.to_string(),
    })
}
