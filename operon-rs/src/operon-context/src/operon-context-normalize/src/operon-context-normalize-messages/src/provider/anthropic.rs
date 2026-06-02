//! Anthropic message normalization and denormalization.
//!
//! Anthropic message wire format differs from OpenAI:
//! - `system` is top-level, not a message role in the messages array.
//! - message `content` can be string or array of typed blocks.
//! - tool calls are `type: "tool_use"` blocks.
//! - tool results are `type: "tool_result"` blocks.
//! - reasoning is `type: "thinking"` blocks.

use operon_context_normalize_reasoning::{
    denormalize_reasoning, normalize_reasoning, Provider as ReasoningProvider,
};
use operon_context_normalize_tools::{
    denormalize_result as denormalize_tool_result, normalize as normalize_tool_call,
    Provider as ToolProvider, ToolCallId, ToolContent, ToolResult,
};
use serde_json::{json, Value};

use crate::error::{MessageNormalizeError, Result};
use crate::stop_reason::{denormalize_stop_reason, normalize_stop_reason};
use crate::types::{
    ContentBlock, ConversationMessage, DocumentBlock, DocumentSource, ImageBlock, ImageSource,
    MessageRole,
};

const PROVIDER: &str = "Anthropic";

/// Normalize an Anthropic wire message (or top-level `system` field wrapper)
/// into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    if raw.get("system").is_some() {
        return normalize_system_wrapper(raw);
    }

    let role =
        raw.get("role")
            .and_then(Value::as_str)
            .ok_or(MessageNormalizeError::MissingField {
                field: "role",
                provider: PROVIDER,
            })?;

    let message_role = match role {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "system" => MessageRole::System,
        other => {
            return Err(MessageNormalizeError::UnknownRole {
                role: other.to_string(),
                provider: PROVIDER,
            })
        }
    };

    if message_role == MessageRole::System {
        let text = raw.get("content").and_then(Value::as_str).ok_or(
            MessageNormalizeError::MissingField {
                field: "content",
                provider: PROVIDER,
            },
        )?;
        return Ok(ConversationMessage::system(text));
    }

    let content = parse_anthropic_content(raw.get("content"), PROVIDER)?;
    let stop_reason = raw
        .get("stop_reason")
        .and_then(Value::as_str)
        .map(|s| normalize_stop_reason(s, &crate::provider::Provider::Anthropic));

    Ok(ConversationMessage {
        role: message_role,
        content,
        stop_reason,
    })
}

/// Denormalize canonical messages into Anthropic wire bundle:
/// `{ "messages": [...], "system": <string-or-null> }`.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    let mut wire_messages = Vec::new();
    let mut system_text_parts = Vec::new();

    for msg in msgs {
        match msg.role {
            MessageRole::System => {
                system_text_parts.push(render_system_text(&msg.content, PROVIDER)?);
            }
            MessageRole::User | MessageRole::Assistant | MessageRole::Tool => {
                let role_str = if msg.role == MessageRole::Assistant {
                    "assistant"
                } else {
                    "user"
                };

                let content_blocks = render_anthropic_content_blocks(&msg.content, PROVIDER)?;
                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String(role_str.to_string()));
                obj.insert("content".to_string(), Value::Array(content_blocks));

                if msg.role == MessageRole::Assistant {
                    if let Some(stop) = &msg.stop_reason {
                        obj.insert(
                            "stop_reason".to_string(),
                            Value::String(
                                denormalize_stop_reason(
                                    stop,
                                    &crate::provider::Provider::Anthropic,
                                )
                                .to_string(),
                            ),
                        );
                    }
                }

                wire_messages.push(Value::Object(obj));
            }
        }
    }

    let system_value = if system_text_parts.is_empty() {
        Value::Null
    } else {
        Value::String(system_text_parts.join("\n\n"))
    };

    Ok(json!({
        "messages": wire_messages,
        "system": system_value,
    }))
}

fn normalize_system_wrapper(raw: Value) -> Result<ConversationMessage> {
    let system = raw
        .get("system")
        .ok_or(MessageNormalizeError::MissingField {
            field: "system",
            provider: PROVIDER,
        })?;

    let text = match system {
        Value::String(s) => s.to_string(),
        Value::Array(arr) => {
            let first_text = arr.iter().find_map(|item| {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    return item.get("text").and_then(Value::as_str).map(str::to_string);
                }
                None
            });
            first_text.ok_or(MessageNormalizeError::MissingField {
                field: "system[].text",
                provider: PROVIDER,
            })?
        }
        _ => {
            return Err(MessageNormalizeError::UnknownShape {
                provider: PROVIDER,
                detail: "expected `system` as string or array of text blocks".to_string(),
            })
        }
    };

    Ok(ConversationMessage::system(text))
}

fn parse_anthropic_content(
    raw_content: Option<&Value>,
    provider_name: &'static str,
) -> Result<Vec<ContentBlock>> {
    match raw_content {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => Ok(vec![ContentBlock::Text(s.to_string())]),
        Some(Value::Array(arr)) => {
            let mut out = Vec::new();
            for block in arr {
                let block_type = block.get("type").and_then(Value::as_str).ok_or(
                    MessageNormalizeError::MissingField {
                        field: "content[].type",
                        provider: provider_name,
                    },
                )?;

                match block_type {
                    "text" => {
                        let text = block.get("text").and_then(Value::as_str).ok_or(
                            MessageNormalizeError::MissingField {
                                field: "content[].text",
                                provider: provider_name,
                            },
                        )?;
                        out.push(ContentBlock::Text(text.to_string()));
                    }
                    "image" => out.push(ContentBlock::Image(parse_anthropic_image_block(
                        block,
                        provider_name,
                    )?)),
                    "document" => out.push(ContentBlock::Document(parse_anthropic_document_block(
                        block,
                        provider_name,
                    )?)),
                    "tool_use" => {
                        let tool = normalize_tool_call(block.clone(), &ToolProvider::Anthropic)
                            .map_err(|e| map_tool_err(e, provider_name))?;
                        out.push(ContentBlock::ToolCall(tool));
                    }
                    "tool_result" => {
                        let tool_result = parse_anthropic_tool_result(block, provider_name)?;
                        out.push(ContentBlock::ToolResult(tool_result));
                    }
                    "thinking" => {
                        let reasoning =
                            normalize_reasoning(block.clone(), &ReasoningProvider::Anthropic)
                                .map_err(|e| map_reasoning_err(e, provider_name))?;
                        for rb in reasoning {
                            out.push(ContentBlock::Reasoning(rb));
                        }
                    }
                    other => {
                        return Err(MessageNormalizeError::UnsupportedContentType {
                            provider: provider_name,
                            detail: format!("unsupported Anthropic content block type `{other}`"),
                        })
                    }
                }
            }
            Ok(out)
        }
        Some(other) => Err(MessageNormalizeError::UnknownShape {
            provider: provider_name,
            detail: format!("expected `content` as string or array, found: {other}"),
        }),
    }
}

fn parse_anthropic_image_block(block: &Value, provider_name: &'static str) -> Result<ImageBlock> {
    let source = block
        .get("source")
        .ok_or(MessageNormalizeError::MissingField {
            field: "content[].source",
            provider: provider_name,
        })?;

    let source_type =
        source
            .get("type")
            .and_then(Value::as_str)
            .ok_or(MessageNormalizeError::MissingField {
                field: "content[].source.type",
                provider: provider_name,
            })?;

    let image_source = match source_type {
        "base64" => {
            let media_type = source.get("media_type").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.media_type",
                    provider: provider_name,
                },
            )?;
            let data = source.get("data").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.data",
                    provider: provider_name,
                },
            )?;
            ImageSource::Base64 {
                media_type: media_type.to_string(),
                data: data.to_string(),
            }
        }
        "url" => {
            let url = source.get("url").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.url",
                    provider: provider_name,
                },
            )?;
            ImageSource::Url(url.to_string())
        }
        other => {
            return Err(MessageNormalizeError::UnsupportedContentType {
                provider: provider_name,
                detail: format!("unsupported Anthropic image source type `{other}`"),
            })
        }
    };

    Ok(ImageBlock {
        source: image_source,
    })
}

fn parse_anthropic_document_block(
    block: &Value,
    provider_name: &'static str,
) -> Result<DocumentBlock> {
    let source = block
        .get("source")
        .ok_or(MessageNormalizeError::MissingField {
            field: "content[].source",
            provider: provider_name,
        })?;

    let source_type =
        source
            .get("type")
            .and_then(Value::as_str)
            .ok_or(MessageNormalizeError::MissingField {
                field: "content[].source.type",
                provider: provider_name,
            })?;

    let canonical_source = match source_type {
        "base64" => {
            let media_type = source.get("media_type").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.media_type",
                    provider: provider_name,
                },
            )?;
            let data = source.get("data").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.data",
                    provider: provider_name,
                },
            )?;
            DocumentSource::Base64 {
                media_type: media_type.to_string(),
                data: data.to_string(),
            }
        }
        "url" => {
            let url = source.get("url").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.url",
                    provider: provider_name,
                },
            )?;
            DocumentSource::Url(url.to_string())
        }
        "text" => {
            let text = source.get("text").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "content[].source.text",
                    provider: provider_name,
                },
            )?;
            DocumentSource::Text(text.to_string())
        }
        other => {
            return Err(MessageNormalizeError::UnsupportedContentType {
                provider: provider_name,
                detail: format!("unsupported Anthropic document source type `{other}`"),
            })
        }
    };

    Ok(DocumentBlock {
        source: canonical_source,
        title: block
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn parse_anthropic_tool_result(block: &Value, provider_name: &'static str) -> Result<ToolResult> {
    let call_id = block.get("tool_use_id").and_then(Value::as_str).ok_or(
        MessageNormalizeError::MissingField {
            field: "content[].tool_use_id",
            provider: provider_name,
        },
    )?;

    let content = match block.get("content") {
        None | Some(Value::Null) => ToolContent::Text(String::new()),
        Some(Value::String(s)) => ToolContent::Text(s.to_string()),
        Some(other) => ToolContent::Json(other.clone()),
    };

    let is_error = block
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(ToolResult {
        call_id: ToolCallId(call_id.to_string()),
        name: block
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        content,
        is_error,
    })
}

fn render_anthropic_content_blocks(
    content: &[ContentBlock],
    provider_name: &'static str,
) -> Result<Vec<Value>> {
    let mut out = Vec::new();

    for block in content {
        match block {
            ContentBlock::Text(s) => out.push(json!({"type": "text", "text": s})),
            ContentBlock::Image(img) => {
                let source = match &img.source {
                    ImageSource::Base64 { media_type, data } => {
                        json!({"type": "base64", "media_type": media_type, "data": data})
                    }
                    ImageSource::Url(url) => json!({"type": "url", "url": url}),
                };
                out.push(json!({"type": "image", "source": source}));
            }
            ContentBlock::Document(doc) => {
                let source = match &doc.source {
                    DocumentSource::Base64 { media_type, data } => {
                        json!({"type": "base64", "media_type": media_type, "data": data})
                    }
                    DocumentSource::Url(url) => json!({"type": "url", "url": url}),
                    DocumentSource::Text(text) => json!({"type": "text", "text": text}),
                };
                out.push(json!({
                    "type": "document",
                    "source": source,
                    "title": doc.title
                }));
            }
            ContentBlock::ToolCall(tc) => out.push(json!({
                "type": "tool_use",
                "id": tc.id.0,
                "name": tc.name,
                "input": tc.arguments,
            })),
            ContentBlock::ToolResult(tr) => {
                let wire = denormalize_tool_result(tr, &ToolProvider::Anthropic)
                    .map_err(|e| map_tool_err(e, provider_name))?;
                out.push(wire);
            }
            ContentBlock::Reasoning(rb) => {
                let wire =
                    denormalize_reasoning(std::slice::from_ref(rb), &ReasoningProvider::Anthropic)
                        .map_err(|e| map_reasoning_err(e, provider_name))?;
                if let Value::Array(arr) = wire {
                    for item in arr {
                        out.push(item);
                    }
                } else {
                    return Err(MessageNormalizeError::UnknownShape {
                        provider: provider_name,
                        detail: "expected Anthropic reasoning denormalization to return array"
                            .to_string(),
                    });
                }
            }
        }
    }

    Ok(out)
}

fn render_system_text(content: &[ContentBlock], provider_name: &'static str) -> Result<String> {
    let mut text_parts = Vec::new();
    for block in content {
        match block {
            ContentBlock::Text(s) => text_parts.push(s.clone()),
            _ => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: "system messages may only include text blocks".to_string(),
                })
            }
        }
    }
    Ok(text_parts.join("\n\n"))
}

fn map_tool_err(
    err: operon_context_normalize_tools::ToolNormalizeError,
    provider_name: &'static str,
) -> MessageNormalizeError {
    match err {
        operon_context_normalize_tools::ToolNormalizeError::MissingField { field, .. } => {
            MessageNormalizeError::MissingField {
                field,
                provider: provider_name,
            }
        }
        other => MessageNormalizeError::UnsupportedContentType {
            provider: provider_name,
            detail: other.to_string(),
        },
    }
}

fn map_reasoning_err(
    err: operon_context_normalize_reasoning::ReasoningNormalizeError,
    provider_name: &'static str,
) -> MessageNormalizeError {
    match err {
        operon_context_normalize_reasoning::ReasoningNormalizeError::MissingField {
            field, ..
        } => MessageNormalizeError::MissingField {
            field,
            provider: provider_name,
        },
        other => MessageNormalizeError::UnsupportedContentType {
            provider: provider_name,
            detail: other.to_string(),
        },
    }
}
