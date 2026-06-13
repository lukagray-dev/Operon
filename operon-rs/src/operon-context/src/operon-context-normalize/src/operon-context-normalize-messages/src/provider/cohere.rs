//! Cohere v2 message normalization and denormalization.
//!
//! Cohere v2 uses OpenAI-like roles and supports content as string or typed
//! block arrays. It does not support image blocks in messages.

use serde_json::{json, Value};

use crate::error::{MessageNormalizeError, Result};
use crate::stop_reason::normalize_stop_reason;
use crate::types::{ContentBlock, ConversationMessage, MessageRole};

const PROVIDER: &str = "Cohere";

/// Normalize a Cohere wire message or response envelope into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    let (message_value, finish_reason) = if raw.get("message").is_some() {
        let msg = raw
            .get("message")
            .cloned()
            .ok_or(MessageNormalizeError::MissingField {
                field: "message",
                provider: PROVIDER,
            })?;
        let fr = raw
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        (msg, fr)
    } else {
        let fr = raw
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        (raw, fr)
    };

    if message_value.get("tool_calls").is_some() {
        return Err(MessageNormalizeError::UnsupportedContentType {
            provider: PROVIDER,
            detail: "tool blocks are not supported under tag protocol".to_string(),
        });
    }

    let role_str = message_value.get("role").and_then(Value::as_str).ok_or(
        MessageNormalizeError::MissingField {
            field: "role",
            provider: PROVIDER,
        },
    )?;

    let role = match role_str {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" => MessageRole::System,
        "tool" => MessageRole::Tool,
        other => {
            return Err(MessageNormalizeError::UnknownRole {
                role: other.to_string(),
                provider: PROVIDER,
            })
        }
    };

    if role == MessageRole::Tool {
        return Err(MessageNormalizeError::UnsupportedContentType {
            provider: PROVIDER,
            detail: "tool role is not supported under tag protocol".to_string(),
        });
    }

    let content = parse_cohere_content(message_value.get("content"))?;

    let mut out = ConversationMessage {
        role,
        content,
        stop_reason: None,
    };

    if let Some(fr) = finish_reason {
        out.stop_reason = Some(normalize_stop_reason(
            &fr,
            &crate::provider::Provider::Cohere,
        ));
    }

    Ok(out)
}

/// Denormalize canonical messages into Cohere wire bundle:
/// `{ "messages": [...], "system": null }`.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    let mut wire_messages = Vec::new();

    for msg in msgs {
        match msg.role {
            MessageRole::Tool => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "Tool messages must be mapped to User messages before serialization".to_string(),
                });
            }
            MessageRole::User | MessageRole::Assistant | MessageRole::System => {
                let mut basic_blocks = Vec::new();

                for block in &msg.content {
                    match block {
                        other => basic_blocks.push(other.clone()),
                    }
                }

                let content_value = render_cohere_content(&basic_blocks)?;
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::System => "system",
                    MessageRole::Tool => "tool",
                };

                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String(role.to_string()));
                obj.insert("content".to_string(), content_value);
                // NOTE: We do not serialize stop_reason / finish_reason back into the messages list
                // for the API request payload because providers only accept those fields in model outputs.
                wire_messages.push(Value::Object(obj));
            }
        }
    }

    Ok(json!({
        "messages": wire_messages,
        "system": Value::Null,
    }))
}

fn parse_cohere_content(raw: Option<&Value>) -> Result<Vec<ContentBlock>> {
    match raw {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => Ok(vec![ContentBlock::Text(s.to_string())]),
        Some(Value::Array(arr)) => {
            let mut out = Vec::new();
            for block in arr {
                let ty = block.get("type").and_then(Value::as_str).ok_or(
                    MessageNormalizeError::MissingField {
                        field: "content[].type",
                        provider: PROVIDER,
                    },
                )?;
                match ty {
                    "text" => {
                        let text = block.get("text").and_then(Value::as_str).ok_or(
                            MessageNormalizeError::MissingField {
                                field: "content[].text",
                                provider: PROVIDER,
                            },
                        )?;
                        out.push(ContentBlock::Text(text.to_string()));
                    }
                    "image" | "image_url" => {
                        return Err(MessageNormalizeError::UnsupportedContentType {
                            provider: PROVIDER,
                            detail: "Cohere v2 does not support image content blocks".to_string(),
                        })
                    }
                    other => {
                        return Err(MessageNormalizeError::UnsupportedContentType {
                            provider: PROVIDER,
                            detail: format!("unsupported Cohere content block type `{other}`"),
                        })
                    }
                }
            }
            Ok(out)
        }
        Some(other) => Err(MessageNormalizeError::UnknownShape {
            provider: PROVIDER,
            detail: format!("expected content string/array/null, found: {other}"),
        }),
    }
}


fn render_cohere_content(blocks: &[ContentBlock]) -> Result<Value> {
    if blocks.is_empty() {
        return Ok(Value::String(String::new()));
    }

    if blocks.len() == 1 {
        if let ContentBlock::Text(s) = &blocks[0] {
            return Ok(Value::String(s.clone()));
        }
    }

    let mut arr = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::Text(s) => arr.push(json!({"type": "text", "text": s})),
            ContentBlock::Image(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "Cohere v2 does not support image content blocks".to_string(),
                })
            }
            ContentBlock::Document(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "Cohere v2 does not support document content blocks".to_string(),
                })
            }
            ContentBlock::ToolCall(_) | ContentBlock::ToolResult(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "Tool messages must be mapped to User messages before serialization".to_string(),
                })
            }
            ContentBlock::Reasoning(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "Cohere v2 does not expose reasoning content blocks".to_string(),
                })
            }
        }
    }
    Ok(Value::Array(arr))
}


