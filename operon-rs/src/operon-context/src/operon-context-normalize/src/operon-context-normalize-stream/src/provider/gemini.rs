//! Gemini streaming parser.

use serde_json::Value;

use crate::error::{Result, StreamNormalizeError};
use crate::types::StreamEvent;

const PROVIDER: &str = "Gemini";

/// Parse one Gemini SSE payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    let raw: Value =
        serde_json::from_str(line).map_err(|source| StreamNormalizeError::MalformedJson {
            provider: PROVIDER,
            source,
        })?;

    parse_value(raw)
}

/// Parse one Gemini response chunk value.
pub fn parse_value(raw: Value) -> Result<Vec<StreamEvent>> {
    let mut events = Vec::new();

    if let Some(candidates) = raw.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array)
            {
                parse_parts(parts, &mut events)?;
            }

            if let Some(finish_reason) = candidate
                .get("finishReason")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::StopReason {
                    raw: finish_reason.to_string(),
                });
            }
        }
    } else if let Some(parts) = raw.get("parts").and_then(Value::as_array) {
        parse_parts(parts, &mut events)?;
    }

    if let Some(usage) = raw.get("usageMetadata") {
        events.push(StreamEvent::UsageMeta { raw: usage.clone() });
    }

    if events.is_empty() {
        return Err(StreamNormalizeError::UnknownEventType {
            event_type: "Gemini chunk contained no supported stream fields".to_string(),
            provider: PROVIDER,
        });
    }

    Ok(events)
}

fn parse_parts(parts: &[Value], events: &mut Vec<StreamEvent>) -> Result<()> {
    for (index, part) in parts.iter().enumerate() {
        if let Some(signature) = part
            .get("thoughtSignature")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            events.push(StreamEvent::ReasoningSignature {
                signature: signature.to_string(),
            });
        }

        if part.get("thought").and_then(Value::as_bool) == Some(true) {
            if let Some(text) = part
                .get("text")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::ReasoningDelta {
                    text: text.to_string(),
                });
            }
            continue;
        }

        if let Some(function_call) = part.get("functionCall") {
            let name = function_call.get("name").and_then(Value::as_str).ok_or(
                StreamNormalizeError::MissingField {
                    field: "parts[].functionCall.name",
                    provider: PROVIDER,
                },
            )?;
            let arguments = function_call
                .get("args")
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default()));

            events.push(StreamEvent::ToolCallComplete {
                index,
                id: None,
                name: name.to_string(),
                arguments,
            });
            continue;
        }

        if let Some(text) = part
            .get("text")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            events.push(StreamEvent::TextDelta {
                text: text.to_string(),
            });
        }
    }

    Ok(())
}
