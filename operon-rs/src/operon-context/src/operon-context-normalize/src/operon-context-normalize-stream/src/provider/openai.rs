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

            let reasoning = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .or_else(|| delta.get("thought"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty());

            if let Some(text) = reasoning {
                events.push(StreamEvent::ReasoningDelta {
                    text: text.to_string(),
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

            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for (array_idx, tool_call) in tool_calls.iter().enumerate() {
                    let index = tool_call
                        .get("index")
                        .and_then(Value::as_u64)
                        .map(|v| v as usize)
                        .unwrap_or(array_idx);

                    let id = tool_call
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let name = tool_call
                        .get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_string);

                    if id.is_some() || name.is_some() {
                        events.push(StreamEvent::ToolCallStart { index, id, name });
                    }

                    if let Some(arguments) = tool_call
                        .get("function")
                        .and_then(|function| function.get("arguments"))
                        .and_then(Value::as_str)
                        .filter(|value| !value.is_empty())
                    {
                        events.push(StreamEvent::ToolCallDelta {
                            index,
                            arguments_fragment: arguments.to_string(),
                        });
                    }
                }
            }

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
    }

    if let Some(usage) = raw.get("usage") {
        events.push(StreamEvent::UsageMeta { raw: usage.clone() });
    }

    if !events.is_empty() {
        return Ok(events);
    }

    Err(StreamNormalizeError::UnknownEventType {
        event_type: "missing `choices`/`usage` in OpenAI-compatible stream payload".to_string(),
        provider,
    })
}
