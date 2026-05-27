//! OpenRouter streaming parser with shape detection.

use serde_json::Value;

use crate::error::{Result, StreamNormalizeError};
use crate::types::StreamEvent;

use super::{anthropic, openai};

const PROVIDER: &str = "OpenRouter";

/// Parse one OpenRouter stream payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    let raw: Value = serde_json::from_str(line).map_err(|source| StreamNormalizeError::MalformedJson {
        provider: PROVIDER,
        source,
    })?;

    parse_value(raw)
}

fn parse_value(raw: Value) -> Result<Vec<StreamEvent>> {
    if raw.get("choices").is_some() {
        return openai::parse_value_with_provider(raw, PROVIDER);
    }

    if looks_like_anthropic_chunk(&raw) {
        return anthropic::parse_value(raw);
    }

    let openai_attempt = openai::parse_value_with_provider(raw.clone(), PROVIDER);
    if let Ok(events) = openai_attempt {
        if !events.is_empty() {
            return Ok(events);
        }
    }

    let anthropic_attempt = anthropic::parse_value(raw.clone());
    if let Ok(events) = anthropic_attempt {
        if !events.is_empty() {
            return Ok(events);
        }
    }

    let keys = raw
        .as_object()
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    Err(StreamNormalizeError::UnknownEventType {
        event_type: format!("unknown OpenRouter stream shape with keys: {:?}", keys),
        provider: PROVIDER,
    })
}

fn looks_like_anthropic_chunk(raw: &Value) -> bool {
    if raw.get("content_block").is_some() {
        return true;
    }

    match raw.get("type").and_then(Value::as_str) {
        Some(
            "message_start"
            | "content_block_start"
            | "content_block_delta"
            | "content_block_stop"
            | "message_delta"
            | "message_stop"
            | "ping",
        ) => true,
        _ => false,
    }
}
