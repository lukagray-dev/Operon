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

        "tool-call-start" | "tool-call-delta" | "tool-call-end" => {
            // Under the plain-text tag protocol, we ignore tool calls on the stream.
            Ok(Vec::new())
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
