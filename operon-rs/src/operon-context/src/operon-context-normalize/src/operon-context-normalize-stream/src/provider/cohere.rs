//! Cohere streaming parser.

use serde_json::Value;

use crate::error::{Result, StreamNormalizeError};
use crate::types::StreamEvent;

const PROVIDER: &str = "Cohere";

/// Parse one Cohere stream payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    let raw: Value =
        serde_json::from_str(line).map_err(|source| StreamNormalizeError::MalformedJson {
            provider: PROVIDER,
            source,
        })?;

    let event_type =
        raw.get("type")
            .and_then(Value::as_str)
            .ok_or(StreamNormalizeError::MissingField {
                field: "type",
                provider: PROVIDER,
            })?;

    match event_type {
        "message-start" => Ok(vec![StreamEvent::StreamStart { model: None }]),
        "content-start" | "content-end" | "citation-start" | "citation-end" => Ok(Vec::new()),

        "content-delta" => {
            let text = raw
                .get("delta")
                .and_then(|delta| delta.get("message"))
                .and_then(|message| message.get("content"))
                .and_then(|content| content.get("text"))
                .and_then(Value::as_str)
                .ok_or(StreamNormalizeError::MissingField {
                    field: "delta.message.content.text",
                    provider: PROVIDER,
                })?;

            Ok(vec![StreamEvent::TextDelta {
                text: text.to_string(),
            }])
        }

        "tool-call-start" => {
            let index = raw.get("index").and_then(Value::as_u64).ok_or(
                StreamNormalizeError::MissingField {
                    field: "index",
                    provider: PROVIDER,
                },
            )? as usize;

            let tool_calls = raw
                .get("delta")
                .and_then(|delta| delta.get("message"))
                .and_then(|message| message.get("tool_calls"))
                .ok_or(StreamNormalizeError::MissingField {
                    field: "delta.message.tool_calls",
                    provider: PROVIDER,
                })?;

            let tool_call = if let Some(array) = tool_calls.as_array() {
                array.first().ok_or(StreamNormalizeError::MissingField {
                    field: "delta.message.tool_calls[0]",
                    provider: PROVIDER,
                })?
            } else {
                tool_calls
            };

            let id = tool_call
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let name = tool_call
                .get("function")
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string);

            Ok(vec![StreamEvent::ToolCallStart { index, id, name }])
        }

        "tool-call-delta" => {
            let index = raw.get("index").and_then(Value::as_u64).ok_or(
                StreamNormalizeError::MissingField {
                    field: "index",
                    provider: PROVIDER,
                },
            )? as usize;

            let arguments = raw
                .get("delta")
                .and_then(|delta| delta.get("message"))
                .and_then(|message| message.get("tool_calls"))
                .and_then(|tool_calls| {
                    if let Some(array) = tool_calls.as_array() {
                        array.first().cloned()
                    } else {
                        Some(tool_calls.clone())
                    }
                })
                .and_then(|tool_call| tool_call.get("function").cloned())
                .and_then(|function| function.get("arguments").cloned())
                .and_then(|value| value.as_str().map(str::to_string))
                .ok_or(StreamNormalizeError::MissingField {
                    field: "delta.message.tool_calls.function.arguments",
                    provider: PROVIDER,
                })?;

            Ok(vec![StreamEvent::ToolCallDelta {
                index,
                arguments_fragment: arguments,
            }])
        }

        "tool-call-end" => {
            let index = raw.get("index").and_then(Value::as_u64).ok_or(
                StreamNormalizeError::MissingField {
                    field: "index",
                    provider: PROVIDER,
                },
            )? as usize;

            Ok(vec![StreamEvent::ToolCallEnd { index }])
        }

        "message-end" => {
            let mut events = Vec::new();
            if let Some(stop_reason) = raw
                .get("delta")
                .and_then(|delta| delta.get("finish_reason"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::StopReason {
                    raw: stop_reason.to_string(),
                });
            }

            if let Some(usage) = raw.get("delta").and_then(|delta| delta.get("usage")) {
                events.push(StreamEvent::UsageMeta { raw: usage.clone() });
            }

            Ok(events)
        }

        "ping" => Ok(vec![StreamEvent::Ping]),

        other => Err(StreamNormalizeError::UnknownEventType {
            event_type: other.to_string(),
            provider: PROVIDER,
        }),
    }
}
