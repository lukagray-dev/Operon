// runner/message_build.rs — Pure helper functions for message construction and serialization.
//
// These free functions have no dependency on `SessionRunner` fields.
// They build conversation messages, extract usage records from provider
// responses, generate session IDs, and format tool results for the event bus.

use operon_context::{
    ContentBlock, ConversationMessage, TokenBudget, ToolCall, ToolContent, ToolResult, UsageRecord,
};
use operon_events::SessionEvent;

use crate::http::StreamResult;

// ─────────────────────────────────────────────────────────────────────────────
// Message construction
// ─────────────────────────────────────────────────────────────────────────────

/// Builds the user turn's content blocks: attached images first (if any),
/// then the file-path references inlined as text, then the user's own text.
pub fn build_user_message(
    text: &str,
    image_blocks: Vec<ContentBlock>,
    file_paths: &[std::path::PathBuf],
) -> Vec<ContentBlock> {
    let mut blocks = image_blocks;

    let mut text_parts = Vec::new();
    if !text.is_empty() {
        text_parts.push(text.to_string());
    }
    for path in file_paths {
        text_parts.push(format!("[Attached file: {}]", path.display()));
    }

    if !text_parts.is_empty() {
        blocks.push(ContentBlock::Text(text_parts.join("\n")));
    } else if blocks.is_empty() {
        blocks.push(ContentBlock::Text(text.to_string()));
    }

    blocks
}

/// Build a `ConversationMessage` from a fully assembled `StreamResult`.
pub(super) fn build_assistant_message(result: &StreamResult) -> ConversationMessage {
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

// ─────────────────────────────────────────────────────────────────────────────
// Usage / token helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract a `UsageRecord` from a raw usage metadata JSON value.
///
/// Handles both Anthropic and OpenAI usage shapes:
///   - Anthropic: `{ "input_tokens": N, "output_tokens": N, "cache_read_input_tokens": N, ... }`
///   - OpenAI:    `{ "prompt_tokens": N, "completion_tokens": N }`
///
/// Returns `None` if the required fields are absent.
pub(super) fn extract_usage_record(
    raw: &serde_json::Value,
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

/// Generate a unique session ID using the current nanosecond timestamp in hex.
pub(super) fn generate_session_id() -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    format!("{nanos:x}")
}

/// Build the context gauge event from the current token state.
pub(super) fn context_usage_event(
    token_budget: &TokenBudget,
    current_context_tokens: usize,
) -> SessionEvent {
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

// ─────────────────────────────────────────────────────────────────────────────
// Tool result helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Construct the opaque error result we return to the model when policy blocks a call.
pub(super) fn opaque_permission_denied_result(call: &ToolCall) -> ToolResult {
    ToolResult {
        call_id: call.id.clone(),
        name: call.name.clone(),
        content: ToolContent::Text("Tool not available.".to_string()),
        is_error: true,
    }
}

/// Convert a tool result into the serialized content string emitted on the event bus.
pub(super) fn tool_result_content_json(result: &ToolResult) -> String {
    match &result.content {
        ToolContent::Text(text) => text.clone(),
        ToolContent::Json(value) => value.to_string(),
    }
}
