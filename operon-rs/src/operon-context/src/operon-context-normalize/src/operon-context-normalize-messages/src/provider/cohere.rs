//! Cohere v2 message normalization and denormalization.
//!
//! Cohere v2 uses OpenAI-like roles and supports content as string or typed
//! block arrays. It does not support image blocks in messages.

use operon_context_normalize_tools::{
    denormalize_result as denormalize_tool_result, normalize as normalize_tool_call,
    Provider as ToolProvider, ToolCallId, ToolContent, ToolResult,
};
use serde_json::{json, Value};

use crate::error::{MessageNormalizeError, Result};
use crate::stop_reason::{denormalize_stop_reason, normalize_stop_reason};
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

    let mut content = if role == MessageRole::Tool {
        vec![ContentBlock::ToolResult(parse_cohere_tool_result(
            &message_value,
        )?)]
    } else {
        parse_cohere_content(message_value.get("content"))?
    };

    if role == MessageRole::Assistant {
        if let Some(tool_calls) = message_value.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                // Cohere tool_calls are OpenAI-shaped in v2 messages.
                let tool_call =
                    normalize_tool_call(tc.clone(), &ToolProvider::OpenAI).map_err(map_tool_err)?;
                content.push(ContentBlock::ToolCall(tool_call));
            }
        }
    }

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
                let tool_results = extract_tool_results(&msg.content)?;
                for tr in tool_results {
                    let wire =
                        denormalize_tool_result(tr, &ToolProvider::Cohere).map_err(map_tool_err)?;
                    wire_messages.push(wire);
                }
            }
            MessageRole::User | MessageRole::Assistant | MessageRole::System => {
                let mut tool_calls = Vec::new();
                let mut basic_blocks = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::ToolCall(tc) => {
                            tool_calls.push(json!({
                                "id": tc.id.0,
                                "type": "tool_call",
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments.to_string()
                                }
                            }));
                        }
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
                if !tool_calls.is_empty() {
                    obj.insert("tool_calls".to_string(), Value::Array(tool_calls));
                }
                if msg.role == MessageRole::Assistant {
                    if let Some(stop) = &msg.stop_reason {
                        obj.insert(
                            "finish_reason".to_string(),
                            Value::String(
                                denormalize_stop_reason(stop, &crate::provider::Provider::Cohere)
                                    .to_string(),
                            ),
                        );
                    }
                }
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

fn parse_cohere_tool_result(message: &Value) -> Result<ToolResult> {
    let tool_call_id = message.get("tool_call_id").and_then(Value::as_str).ok_or(
        MessageNormalizeError::MissingField {
            field: "tool_call_id",
            provider: PROVIDER,
        },
    )?;

    let content = match message.get("content") {
        None | Some(Value::Null) => ToolContent::Text(String::new()),
        Some(Value::String(s)) => ToolContent::Text(s.to_string()),
        Some(Value::Array(arr)) => {
            let joined = arr
                .iter()
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            if joined.is_empty() {
                ToolContent::Json(Value::Array(arr.clone()))
            } else {
                ToolContent::Text(joined)
            }
        }
        Some(other) => ToolContent::Json(other.clone()),
    };

    Ok(ToolResult {
        call_id: ToolCallId(tool_call_id.to_string()),
        name: message
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        content,
        is_error: false,
    })
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
                    detail: "tool blocks must be represented via `tool_calls` or `role=tool` messages for Cohere".to_string(),
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

fn extract_tool_results<'a>(blocks: &'a [ContentBlock]) -> Result<Vec<&'a ToolResult>> {
    let mut out = Vec::new();
    for block in blocks {
        match block {
            ContentBlock::ToolResult(tr) => out.push(tr),
            _ => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "role=tool messages may only contain ToolResult blocks".to_string(),
                })
            }
        }
    }
    if out.is_empty() {
        return Err(MessageNormalizeError::MissingField {
            field: "tool_result",
            provider: PROVIDER,
        });
    }
    Ok(out)
}

fn map_tool_err(err: operon_context_normalize_tools::ToolNormalizeError) -> MessageNormalizeError {
    match err {
        operon_context_normalize_tools::ToolNormalizeError::MissingField { field, .. } => {
            MessageNormalizeError::MissingField {
                field,
                provider: PROVIDER,
            }
        }
        other => MessageNormalizeError::UnsupportedContentType {
            provider: PROVIDER,
            detail: other.to_string(),
        },
    }
}
