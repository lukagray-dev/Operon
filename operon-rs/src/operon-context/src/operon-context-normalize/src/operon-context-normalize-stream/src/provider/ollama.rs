//! Ollama streaming parser.

use serde_json::Value;

use crate::error::{Result, StreamNormalizeError};
use crate::types::StreamEvent;

use super::openai;

const PROVIDER: &str = "Ollama";

/// Parse one Ollama stream payload line.
///
/// Supports:
/// - OpenAI-compatible `/v1/chat/completions` chunks.
/// - Native `/api/chat` NDJSON chunks.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    if line.trim_start().starts_with("{\"choices\"") || line.contains("\"choices\":") {
        return openai::parse_line_with_provider(line, PROVIDER);
    }

    let raw: Value =
        serde_json::from_str(line).map_err(|source| StreamNormalizeError::MalformedJson {
            provider: PROVIDER,
            source,
        })?;

    parse_native_value(raw)
}

fn parse_native_value(raw: Value) -> Result<Vec<StreamEvent>> {
    let mut events = Vec::new();

    if let Some(message) = raw.get("message") {
        if let Some(thinking) = message
            .get("thinking")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            events.push(StreamEvent::ReasoningDelta {
                text: thinking.to_string(),
            });
        }

        if let Some(content) = message
            .get("content")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            events.push(StreamEvent::TextDelta {
                text: content.to_string(),
            });
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
            for (index, tool_call) in tool_calls.iter().enumerate() {
                let function =
                    tool_call
                        .get("function")
                        .ok_or(StreamNormalizeError::MissingField {
                            field: "message.tool_calls[].function",
                            provider: PROVIDER,
                        })?;

                let name = function.get("name").and_then(Value::as_str).ok_or(
                    StreamNormalizeError::MissingField {
                        field: "message.tool_calls[].function.name",
                        provider: PROVIDER,
                    },
                )?;

                let arguments = function
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Default::default()));

                events.push(StreamEvent::ToolCallComplete {
                    index,
                    id: None,
                    name: name.to_string(),
                    arguments,
                });
            }
        }
    }

    if raw.get("done").and_then(Value::as_bool) == Some(true) {
        if let Some(done_reason) = raw
            .get("done_reason")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            events.push(StreamEvent::StopReason {
                raw: done_reason.to_string(),
            });
        }
    }

    if events.is_empty() {
        return Err(StreamNormalizeError::UnknownEventType {
            event_type: "Ollama chunk contained no supported stream fields".to_string(),
            provider: PROVIDER,
        });
    }

    Ok(events)
}
