//! OpenAI-compatible streaming parser.

use serde_json::Value;

use crate::error::{Result, StreamNormalizeError};
use crate::types::StreamEvent;

const PROVIDER: &str = "OpenAI";

/// Parse one OpenAI stream payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    parse_line_with_provider(line, PROVIDER)
}

/// Parse one OpenAI-compatible stream payload line using a custom provider
/// label in errors (used by delegating provider modules).
pub fn parse_line_with_provider(line: &str, provider: &'static str) -> Result<Vec<StreamEvent>> {
    let raw: Value = serde_json::from_str(line)
        .map_err(|source| StreamNormalizeError::MalformedJson { provider, source })?;
    parse_value_with_provider(raw, provider)
}

/// Parse one OpenAI-compatible stream payload value.
pub fn parse_value_with_provider(raw: Value, provider: &'static str) -> Result<Vec<StreamEvent>> {
    let mut events = Vec::new();

    if let Some(choices) = raw.get("choices").and_then(Value::as_array) {
        for choice in choices {
            let delta = choice
                .get("delta")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();

            if let Some(reasoning) = delta
                .get("reasoning_content")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::ReasoningDelta {
                    text: reasoning.to_string(),
                });
            }

            if let Some(content) = delta
                .get("content")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::TextDelta {
                    text: content.to_string(),
                });
            }

            // Under the plain-text tag protocol, we ignore tool calls on the stream.

            if let Some(finish_reason) = choice
                .get("finish_reason")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::StopReason {
                    raw: finish_reason.to_string(),
                });
            }
        }

        return Ok(events);
    }

    if raw.get("usage").is_some() {
        events.push(StreamEvent::UsageMeta { raw });
        return Ok(events);
    }

    Err(StreamNormalizeError::UnknownEventType {
        event_type: "missing `choices`/`usage` in OpenAI-compatible stream payload".to_string(),
        provider,
    })
}
