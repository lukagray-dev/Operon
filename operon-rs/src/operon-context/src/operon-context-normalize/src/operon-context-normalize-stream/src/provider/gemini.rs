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

            // Hey friend! Google Gemini does not provide a call ID in its REST wire format.
            // We synthesize a unique ID for each tool call so that the GUI / TUI frontend
            // can create a separate workgroup card for each tool call rather than colliding
            // all tool executions into a single card.
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            name.hash(&mut hasher);
            arguments.to_string().hash(&mut hasher);
            index.hash(&mut hasher);
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
                .hash(&mut hasher);
            let id = format!("gemini-call-{:016x}", hasher.finish());

            events.push(StreamEvent::ToolCallComplete {
                index,
                id: Some(id),
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
