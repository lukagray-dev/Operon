//! Gemini message normalization and denormalization.
//!
//! Gemini wire format uses:
//! - top-level `system_instruction`
//! - `contents` array with `role` + `parts`
//! - assistant role value `model`
//! - typed parts for text, thought, function calls/responses, and images

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use operon_context_normalize_reasoning::{
    denormalize_reasoning, normalize_reasoning, Provider as ReasoningProvider,
};
use operon_context_normalize_tools::{
    denormalize_result as denormalize_tool_result, normalize as normalize_tool_call,
    Provider as ToolProvider, ToolCallId, ToolContent, ToolResult,
};
use serde_json::{json, Value};

use crate::error::{MessageNormalizeError, Result};
use crate::stop_reason::normalize_stop_reason;
use crate::types::{ContentBlock, ConversationMessage, ImageBlock, ImageSource, MessageRole};

const PROVIDER: &str = "Gemini";

/// Normalize a Gemini wire message/candidate/system wrapper into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    if raw.get("system_instruction").is_some() {
        return normalize_system_instruction(raw);
    }

    let (message_value, finish_reason) = extract_message_and_finish_reason(raw)?;
    let role_str = message_value.get("role").and_then(Value::as_str).ok_or(
        MessageNormalizeError::MissingField {
            field: "role",
            provider: PROVIDER,
        },
    )?;

    let role = match role_str {
        "user" => MessageRole::User,
        "model" => MessageRole::Assistant,
        "system" => MessageRole::System,
        other => {
            return Err(MessageNormalizeError::UnknownRole {
                role: other.to_string(),
                provider: PROVIDER,
            })
        }
    };

    if role == MessageRole::System {
        let text = extract_system_text_from_parts(message_value.get("parts"))?;
        return Ok(ConversationMessage::system(text));
    }

    let parts = message_value.get("parts").and_then(Value::as_array).ok_or(
        MessageNormalizeError::MissingField {
            field: "parts",
            provider: PROVIDER,
        },
    )?;

    let mut content: Vec<ContentBlock> = Vec::new();
    for part in parts {
        if part.get("thought").and_then(Value::as_bool) == Some(true) {
            let blocks = normalize_reasoning(part.clone(), &ReasoningProvider::Gemini)
                .map_err(map_reasoning_err)?;
            for rb in blocks {
                content.push(ContentBlock::Reasoning(rb));
            }
            continue;
        }

        if part.get("functionCall").is_some() {
            let tool_call =
                normalize_tool_call(part.clone(), &ToolProvider::Gemini).map_err(map_tool_err)?;
            content.push(ContentBlock::ToolCall(tool_call));
            continue;
        }

        if part.get("functionResponse").is_some() {
            content.push(ContentBlock::ToolResult(parse_function_response(part)?));
            continue;
        }

        let inline_opt = part.get("inlineData").or_else(|| part.get("inline_data"));
        if let Some(inline) = inline_opt {
            let mime_type = inline
                .get("mimeType")
                .or_else(|| inline.get("mime_type"))
                .and_then(Value::as_str)
                .ok_or(MessageNormalizeError::MissingField {
                    field: "inlineData.mimeType",
                    provider: PROVIDER,
                })?;
            let data = inline.get("data").and_then(Value::as_str).ok_or(
                MessageNormalizeError::MissingField {
                    field: "inlineData.data",
                    provider: PROVIDER,
                },
            )?;
            content.push(ContentBlock::Image(ImageBlock {
                source: ImageSource::Base64 {
                    media_type: mime_type.to_string(),
                    data: data.to_string(),
                },
            }));
            continue;
        }

        let file_opt = part.get("fileData").or_else(|| part.get("file_data"));
        if let Some(file) = file_opt {
            let uri = file
                .get("fileUri")
                .or_else(|| file.get("file_uri"))
                .and_then(Value::as_str)
                .ok_or(MessageNormalizeError::MissingField {
                    field: "fileData.fileUri",
                    provider: PROVIDER,
                })?;
            content.push(ContentBlock::Image(ImageBlock {
                source: ImageSource::Url(uri.to_string()),
            }));
            continue;
        }

        if let Some(text) = part.get("text").and_then(Value::as_str) {
            content.push(ContentBlock::Text(text.to_string()));
            continue;
        }

        return Err(MessageNormalizeError::UnsupportedContentType {
            provider: PROVIDER,
            detail: format!("unsupported Gemini part shape: {part}"),
        });
    }

    let mut out = ConversationMessage {
        role,
        content,
        stop_reason: None,
    };

    if let Some(fr) = finish_reason {
        out.stop_reason = Some(normalize_stop_reason(
            &fr,
            &crate::provider::Provider::Gemini,
        ));
    }

    Ok(out)
}

/// Denormalize canonical messages into Gemini wire bundle:
/// `{ "messages": <contents-array>, "system": <string-or-null> }`.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    let mut contents = Vec::new();
    let mut system_text_parts = Vec::new();

    for msg in msgs {
        match msg.role {
            MessageRole::System => {
                system_text_parts.push(render_system_text(&msg.content)?);
            }
            MessageRole::Assistant | MessageRole::User | MessageRole::Tool => {
                let role = if msg.role == MessageRole::Assistant {
                    "model"
                } else {
                    "user"
                };
                let parts = render_parts_for_message(msg)?;
                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String(role.to_string()));
                obj.insert("parts".to_string(), Value::Array(parts));
                contents.push(Value::Object(obj));
            }
        }
    }

    let system_value = if system_text_parts.is_empty() {
        Value::Null
    } else {
        Value::String(system_text_parts.join("\n\n"))
    };

    Ok(json!({
        "messages": contents,
        "system": system_value,
    }))
}

fn normalize_system_instruction(raw: Value) -> Result<ConversationMessage> {
    let text = raw
        .get("system_instruction")
        .and_then(|v| v.get("parts"))
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find_map(|p| p.get("text").and_then(Value::as_str))
        })
        .ok_or(MessageNormalizeError::MissingField {
            field: "system_instruction.parts[].text",
            provider: PROVIDER,
        })?;
    Ok(ConversationMessage::system(text))
}

fn extract_message_and_finish_reason(raw: Value) -> Result<(Value, Option<String>)> {
    if let Some(candidates) = raw.get("candidates").and_then(Value::as_array) {
        let first = candidates
            .first()
            .ok_or(MessageNormalizeError::MissingField {
                field: "candidates[0]",
                provider: PROVIDER,
            })?;
        let content = first
            .get("content")
            .ok_or(MessageNormalizeError::MissingField {
                field: "candidates[0].content",
                provider: PROVIDER,
            })?
            .clone();
        let finish_reason = first
            .get("finishReason")
            .and_then(Value::as_str)
            .map(str::to_string);
        return Ok((content, finish_reason));
    }

    let finish_reason = raw
        .get("finishReason")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(content) = raw.get("content") {
        if content.is_object() && raw.get("role").is_none() {
            return Ok((content.clone(), finish_reason));
        }
    }

    Ok((raw, finish_reason))
}

fn extract_system_text_from_parts(parts_value: Option<&Value>) -> Result<String> {
    let parts =
        parts_value
            .and_then(Value::as_array)
            .ok_or(MessageNormalizeError::MissingField {
                field: "parts",
                provider: PROVIDER,
            })?;

    let text = parts
        .iter()
        .find_map(|p| p.get("text").and_then(Value::as_str))
        .ok_or(MessageNormalizeError::MissingField {
            field: "parts[].text",
            provider: PROVIDER,
        })?;

    Ok(text.to_string())
}

fn parse_function_response(part: &Value) -> Result<ToolResult> {
    let fr = part
        .get("functionResponse")
        .ok_or(MessageNormalizeError::MissingField {
            field: "functionResponse",
            provider: PROVIDER,
        })?;

    let name =
        fr.get("name")
            .and_then(Value::as_str)
            .ok_or(MessageNormalizeError::MissingField {
                field: "functionResponse.name",
                provider: PROVIDER,
            })?;

    let response = fr
        .get("response")
        .cloned()
        .unwrap_or_else(|| json!({"content": ""}));

    let content = if let Some(raw_content) = response.get("content") {
        match raw_content {
            Value::String(s) => ToolContent::Text(s.to_string()),
            other => ToolContent::Json(other.clone()),
        }
    } else {
        ToolContent::Json(response.clone())
    };

    let call_id = synthetic_function_response_call_id(name, &response);
    Ok(ToolResult {
        call_id: ToolCallId(call_id),
        name: name.to_string(),
        content,
        is_error: false,
    })
}

fn synthetic_function_response_call_id(name: &str, response: &Value) -> String {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    response.to_string().hash(&mut hasher);
    format!("gemini-fr-{:016x}", hasher.finish())
}

fn render_parts_for_message(msg: &ConversationMessage) -> Result<Vec<Value>> {
    let mut parts = Vec::new();

    for block in &msg.content {
        match block {
            ContentBlock::Text(s) => parts.push(json!({"text": s})),
            ContentBlock::Image(img) => match &img.source {
                ImageSource::Base64 { media_type, data } => {
                    parts.push(json!({
                        "inlineData": {
                            "mimeType": media_type,
                            "data": data
                        }
                    }));
                }
                ImageSource::Url(url) => {
                    parts.push(json!({
                        "fileData": {
                            "mimeType": "image/*",
                            "fileUri": url
                        }
                    }));
                }
            },
            ContentBlock::ToolCall(tc) => {
                parts.push(json!({
                    "functionCall": {
                        "name": tc.name,
                        "args": tc.arguments
                    }
                }));
            }
            ContentBlock::ToolResult(tr) => {
                let wire =
                    denormalize_tool_result(tr, &ToolProvider::Gemini).map_err(map_tool_err)?;
                parts.push(wire);
            }
            ContentBlock::Reasoning(rb) => {
                let wire =
                    denormalize_reasoning(std::slice::from_ref(rb), &ReasoningProvider::Gemini)
                        .map_err(map_reasoning_err)?;
                if let Value::Array(arr) = wire {
                    for p in arr {
                        parts.push(p);
                    }
                } else {
                    return Err(MessageNormalizeError::UnknownShape {
                        provider: PROVIDER,
                        detail: "expected Gemini reasoning denormalization to return array"
                            .to_string(),
                    });
                }
            }
            ContentBlock::Document(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "document blocks are not supported by Gemini message parts".to_string(),
                })
            }
        }
    }

    Ok(parts)
}

fn render_system_text(content: &[ContentBlock]) -> Result<String> {
    let mut text_parts = Vec::new();
    for block in content {
        match block {
            ContentBlock::Text(s) => text_parts.push(s.clone()),
            _ => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "system messages may only include text blocks".to_string(),
                })
            }
        }
    }
    Ok(text_parts.join("\n\n"))
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

fn map_reasoning_err(
    err: operon_context_normalize_reasoning::ReasoningNormalizeError,
) -> MessageNormalizeError {
    match err {
        operon_context_normalize_reasoning::ReasoningNormalizeError::MissingField {
            field, ..
        } => MessageNormalizeError::MissingField {
            field,
            provider: PROVIDER,
        },
        other => MessageNormalizeError::UnsupportedContentType {
            provider: PROVIDER,
            detail: other.to_string(),
        },
    }
}
