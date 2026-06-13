//! Anthropic streaming parser.

use serde_json::Value;

use crate::error::{Result, StreamNormalizeError};
use crate::types::StreamEvent;

const PROVIDER: &str = "Anthropic";

/// Parse one Anthropic stream payload line.
pub fn parse_line(line: &str) -> Result<Vec<StreamEvent>> {
    let raw: Value =
        serde_json::from_str(line).map_err(|source| StreamNormalizeError::MalformedJson {
            provider: PROVIDER,
            source,
        })?;

    parse_value(raw)
}

/// Parse one Anthropic stream payload value.
pub fn parse_value(raw: Value) -> Result<Vec<StreamEvent>> {
    let event_type =
        raw.get("type")
            .and_then(Value::as_str)
            .ok_or(StreamNormalizeError::MissingField {
                field: "type",
                provider: PROVIDER,
            })?;

    match event_type {
        "message_start" => {
            let model = raw
                .get("message")
                .and_then(|message| message.get("model"))
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(vec![StreamEvent::StreamStart { model }])
        }

        "content_block_start" => {
            // Under the plain-text tag protocol, we ignore tool use blocks on the stream.
            Ok(Vec::new())
        }

        "content_block_delta" => {
            let delta_type = raw
                .get("delta")
                .and_then(|delta| delta.get("type"))
                .and_then(Value::as_str)
                .ok_or(StreamNormalizeError::MissingField {
                    field: "delta.type",
                    provider: PROVIDER,
                })?;

            match delta_type {
                "text_delta" => {
                    let text = raw
                        .get("delta")
                        .and_then(|delta| delta.get("text"))
                        .and_then(Value::as_str)
                        .ok_or(StreamNormalizeError::MissingField {
                            field: "delta.text",
                            provider: PROVIDER,
                        })?;
                    Ok(vec![StreamEvent::TextDelta {
                        text: text.to_string(),
                    }])
                }
                "input_json_delta" => {
                    // Ignore tool call input json deltas
                    Ok(Vec::new())
                }
                "thinking_delta" => {
                    let thinking = raw
                        .get("delta")
                        .and_then(|delta| delta.get("thinking"))
                        .and_then(Value::as_str)
                        .ok_or(StreamNormalizeError::MissingField {
                            field: "delta.thinking",
                            provider: PROVIDER,
                        })?;
                    Ok(vec![StreamEvent::ReasoningDelta {
                        text: thinking.to_string(),
                    }])
                }
                "signature_delta" => {
                    let signature = raw
                        .get("delta")
                        .and_then(|delta| delta.get("signature"))
                        .and_then(Value::as_str)
                        .ok_or(StreamNormalizeError::MissingField {
                            field: "delta.signature",
                            provider: PROVIDER,
                        })?;
                    Ok(vec![StreamEvent::ReasoningSignature {
                        signature: signature.to_string(),
                    }])
                }
                other => Err(StreamNormalizeError::UnknownEventType {
                    event_type: format!("content_block_delta.{other}"),
                    provider: PROVIDER,
                }),
            }
        }

        "content_block_stop" => {
            // Ignore content block stop
            Ok(Vec::new())
        }

        "message_delta" => {
            let mut events = Vec::new();

            if let Some(reason) = raw
                .get("delta")
                .and_then(|delta| delta.get("stop_reason"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                events.push(StreamEvent::StopReason {
                    raw: reason.to_string(),
                });
            }

            if let Some(usage) = raw.get("usage") {
                events.push(StreamEvent::UsageMeta { raw: usage.clone() });
            }

            Ok(events)
        }

        "message_stop" => Ok(Vec::new()),
        "ping" => Ok(vec![StreamEvent::Ping]),

        other => Err(StreamNormalizeError::UnknownEventType {
            event_type: other.to_string(),
            provider: PROVIDER,
        }),
    }
}
