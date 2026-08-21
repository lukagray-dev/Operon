//! OpenRouter message normalization and denormalization.
//!
//! OpenRouter proxies multiple underlying providers. On normalize we perform
//! shape detection and delegate either to OpenAI-style or Anthropic-style
//! conversion. On denormalize we always emit OpenAI-style messages.

use serde_json::Value;

use crate::error::{MessageNormalizeError, Result};
use crate::types::ConversationMessage;

use super::{anthropic, openai};

const PROVIDER: &str = "OpenRouter";

use operon_context_normalize_reasoning::Provider as ReasoningProvider;

/// Normalize an OpenRouter wire payload by shape detection.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    if looks_like_openai_shape(&raw) {
        return openai::normalize_message_with_provider_and_reasoning(
            raw,
            PROVIDER,
            Some("reasoning_content"),
            Some(ReasoningProvider::OpenRouter),
        );
    }
    if looks_like_anthropic_shape(&raw) {
        return anthropic::normalize_message(raw);
    }

    let found_keys: Vec<String> = raw
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();

    Err(MessageNormalizeError::UnknownShape {
        provider: PROVIDER,
        detail: format!(
            "could not detect OpenAI or Anthropic message shape; found keys: {:?}",
            found_keys
        ),
    })
}

/// Denormalize canonical messages into OpenRouter bundle.
///
/// OpenRouter accepts OpenAI-style message payloads with reasoning_content support.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    openai::denormalize_messages_with_provider_and_reasoning(
        msgs,
        PROVIDER,
        Some("reasoning_content"),
        Some(ReasoningProvider::OpenRouter),
    )
}

fn looks_like_openai_shape(raw: &Value) -> bool {
    if raw.get("choices").is_some() {
        return true;
    }
    if raw.get("function").is_some() {
        return true;
    }
    if raw.get("tool_call_id").is_some() {
        return true;
    }
    if raw.get("tool_calls").is_some() {
        return true;
    }
    if let Some(role) = raw.get("role").and_then(Value::as_str) {
        if matches!(role, "system" | "tool") {
            return true;
        }
        if role == "assistant"
            && raw
                .get("content")
                .is_none_or(|v| v.is_string() || v.is_null() || looks_like_openai_content_array(v))
        {
            return true;
        }
        if role == "user"
            && raw
                .get("content")
                .is_some_and(|v| v.is_string() || looks_like_openai_content_array(v))
        {
            return true;
        }
    }
    false
}

fn looks_like_openai_content_array(value: &Value) -> bool {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return false,
    };

    // OpenAI-compatible content arrays must be typed blocks.
    for item in arr {
        let block_type = match item.get("type").and_then(Value::as_str) {
            Some(t) => t,
            None => return false,
        };

        match block_type {
            "text" | "input_text" | "output_text" => {
                if item.get("text").and_then(Value::as_str).is_none() {
                    return false;
                }
            }
            "image_url" => {
                if item
                    .get("image_url")
                    .and_then(|v| v.get("url"))
                    .and_then(Value::as_str)
                    .is_none()
                {
                    return false;
                }
            }
            _ => return false,
        }
    }

    true
}

fn looks_like_anthropic_shape(raw: &Value) -> bool {
    if raw.get("system").is_some() {
        return true;
    }
    if raw.get("type").and_then(Value::as_str) == Some("tool_use") {
        return true;
    }
    if let Some(content) = raw.get("content").and_then(Value::as_array) {
        for block in content {
            if let Some(t) = block.get("type").and_then(Value::as_str) {
                if matches!(t, "tool_use" | "tool_result" | "thinking" | "document") {
                    return true;
                }
                if t == "image" && block.get("source").is_some() {
                    return true;
                }
            }
        }
    }
    false
}
