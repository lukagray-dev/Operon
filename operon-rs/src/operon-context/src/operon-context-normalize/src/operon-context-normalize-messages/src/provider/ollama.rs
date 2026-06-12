//! Ollama message normalization and denormalization.
//!
//! Supports:
//! - OpenAI-compatible `/v1/chat/completions` shape (delegates to OpenAI module)
//! - Native `/api/chat` message shape with `thinking` and `done_reason`

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
use crate::types::{ContentBlock, ConversationMessage, ImageBlock, ImageSource, MessageRole};

use super::openai;

const PROVIDER: &str = "Ollama";

/// Normalize an Ollama wire payload into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    if raw.get("choices").is_some() {
        return openai::normalize_message_with_provider(raw, PROVIDER);
    }

    if raw.get("message").is_some() {
        return normalize_native_envelope(raw);
    }

    if looks_like_openai_message_shape(&raw) {
        return openai::normalize_message_with_provider(raw, PROVIDER);
    }

    normalize_native_message(raw, None)
}

/// Denormalize canonical messages into Ollama wire bundle.
///
/// If assistant messages contain reasoning blocks, native `/api/chat` shape is
/// emitted. Otherwise OpenAI-compatible shape is emitted.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    if should_use_native_shape(msgs) {
        denormalize_native_messages(msgs)
    } else {
        openai::denormalize_messages_with_provider(msgs, PROVIDER)
    }
}

fn normalize_native_envelope(raw: Value) -> Result<ConversationMessage> {
    let message = raw
        .get("message")
        .cloned()
        .ok_or(MessageNormalizeError::MissingField {
            field: "message",
            provider: PROVIDER,
        })?;

    let done_reason = raw
        .get("done_reason")
        .and_then(Value::as_str)
        .map(str::to_string);

    normalize_native_message(message, done_reason)
}

fn normalize_native_message(
    raw: Value,
    done_reason: Option<String>,
) -> Result<ConversationMessage> {
    let role_str =
        raw.get("role")
            .and_then(Value::as_str)
            .ok_or(MessageNormalizeError::MissingField {
                field: "role",
                provider: PROVIDER,
            })?;

    let role = match role_str {
        "system" => MessageRole::System,
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        other => {
            return Err(MessageNormalizeError::UnknownRole {
                role: other.to_string(),
                provider: PROVIDER,
            })
        }
    };

    if role == MessageRole::Tool {
        return normalize_native_tool_message(raw);
    }

    let mut content = parse_native_content(raw.get("content"))?;

    if role == MessageRole::Assistant {
        if let Some(thinking) = raw.get("thinking").cloned() {
            let reasoning = normalize_reasoning(thinking, &ReasoningProvider::Ollama)
                .map_err(map_reasoning_err)?;
            let mut combined = Vec::with_capacity(reasoning.len() + content.len());
            for rb in reasoning {
                combined.push(ContentBlock::Reasoning(rb));
            }
            combined.extend(content);
            content = combined;
        }

        if let Some(tool_calls) = raw.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                let candidate = if tc.get("function").is_some() {
                    tc.clone()
                } else {
                    native_tool_call_to_openai_shape(tc)?
                };
                let call =
                    normalize_tool_call(candidate, &ToolProvider::Ollama).map_err(map_tool_err)?;
                content.push(ContentBlock::ToolCall(call));
            }
        }
    }

    let mut out = ConversationMessage {
        role,
        content,
        stop_reason: None,
    };

    let stop_raw = done_reason.or_else(|| {
        raw.get("done_reason")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    if let Some(stop) = stop_raw {
        out.stop_reason = Some(normalize_stop_reason(
            &stop,
            &crate::provider::Provider::Ollama,
        ));
    }

    Ok(out)
}

fn normalize_native_tool_message(raw: Value) -> Result<ConversationMessage> {
    let call_id = raw
        .get("tool_call_id")
        .and_then(Value::as_str)
        .unwrap_or("ollama-tool-call");

    let content = match raw.get("content") {
        None | Some(Value::Null) => ToolContent::Text(String::new()),
        Some(Value::String(s)) => ToolContent::Text(s.to_string()),
        Some(other) => ToolContent::Json(other.clone()),
    };

    let tool_result = ToolResult {
        call_id: ToolCallId(call_id.to_string()),
        name: raw
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        content,
        is_error: false,
        // Since we are parsing a tool result from Ollama's raw wire message format,
        // we do not have (nor do we need) the in-memory read_paths ledger data. Therefore,
        // we default this field to None.
        read_paths: None,
    };

    Ok(ConversationMessage {
        role: MessageRole::Tool,
        content: vec![ContentBlock::ToolResult(tool_result)],
        stop_reason: None,
    })
}

fn parse_native_content(raw_content: Option<&Value>) -> Result<Vec<ContentBlock>> {
    match raw_content {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => Ok(vec![ContentBlock::Text(s.to_string())]),
        Some(Value::Array(arr)) => parse_typed_content_blocks(arr),
        Some(other) => Err(MessageNormalizeError::UnknownShape {
            provider: PROVIDER,
            detail: format!("expected content string/array/null for native Ollama, found: {other}"),
        }),
    }
}

fn parse_typed_content_blocks(arr: &[Value]) -> Result<Vec<ContentBlock>> {
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
            "image_url" => {
                let url = block
                    .get("image_url")
                    .and_then(|v| v.get("url"))
                    .and_then(Value::as_str)
                    .ok_or(MessageNormalizeError::MissingField {
                        field: "content[].image_url.url",
                        provider: PROVIDER,
                    })?;
                out.push(ContentBlock::Image(ImageBlock {
                    source: parse_image_source(url),
                }));
            }
            other => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: format!("unsupported native Ollama content block type `{other}`"),
                })
            }
        }
    }
    Ok(out)
}

fn parse_image_source(url: &str) -> ImageSource {
    if let Some(rest) = url.strip_prefix("data:") {
        if let Some((meta, data)) = rest.split_once(',') {
            if let Some(media_type) = meta.strip_suffix(";base64") {
                return ImageSource::Base64 {
                    media_type: media_type.to_string(),
                    data: data.to_string(),
                };
            }
        }
    }
    ImageSource::Url(url.to_string())
}

fn native_tool_call_to_openai_shape(raw: &Value) -> Result<Value> {
    let name =
        raw.get("name")
            .and_then(Value::as_str)
            .ok_or(MessageNormalizeError::MissingField {
                field: "tool_calls[].name",
                provider: PROVIDER,
            })?;
    let args = raw.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let id = raw
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("ollama-native-call");

    Ok(json!({
        "id": id,
        "type": "function",
        "function": {
            "name": name,
            "arguments": args.to_string()
        }
    }))
}

fn should_use_native_shape(msgs: &[ConversationMessage]) -> bool {
    for msg in msgs {
        if msg.role == MessageRole::Assistant {
            for block in &msg.content {
                if matches!(block, ContentBlock::Reasoning(_)) {
                    return true;
                }
            }
        }
    }
    false
}

fn denormalize_native_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    let mut wire_messages = Vec::new();

    for msg in msgs {
        match msg.role {
            MessageRole::System | MessageRole::User | MessageRole::Assistant => {
                let mut text_image_blocks = Vec::new();
                let mut reasoning_blocks = Vec::new();
                let mut tool_calls = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::Reasoning(rb) => reasoning_blocks.push(rb.clone()),
                        ContentBlock::ToolCall(tc) => tool_calls.push(tc.clone()),
                        other => text_image_blocks.push(other.clone()),
                    }
                }

                let role_str = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };

                let content_value = render_text_and_images(&text_image_blocks)?;
                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String(role_str.to_string()));
                obj.insert("content".to_string(), content_value);

                if !tool_calls.is_empty() {
                    let calls = tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id.0,
                                "function": {
                                    "name": tc.name,
                                    "arguments": tc.arguments
                                }
                            })
                        })
                        .collect();
                    obj.insert("tool_calls".to_string(), Value::Array(calls));
                }

                if !reasoning_blocks.is_empty() {
                    let thinking =
                        denormalize_reasoning(&reasoning_blocks, &ReasoningProvider::Ollama)
                            .map_err(map_reasoning_err)?;
                    obj.insert("thinking".to_string(), thinking);
                }

                if msg.role == MessageRole::Assistant {
                    if let Some(stop) = &msg.stop_reason {
                        obj.insert(
                            "done_reason".to_string(),
                            Value::String(
                                denormalize_stop_reason(stop, &crate::provider::Provider::Ollama)
                                    .to_string(),
                            ),
                        );
                    }
                }

                wire_messages.push(Value::Object(obj));
            }
            MessageRole::Tool => {
                let tool_results = extract_tool_results(&msg.content)?;
                for tr in tool_results {
                    let mut wire =
                        denormalize_tool_result(tr, &ToolProvider::OpenAI).map_err(map_tool_err)?;
                    if let Some(obj) = wire.as_object_mut() {
                        obj.insert("role".to_string(), Value::String("tool".to_string()));
                    }
                    wire_messages.push(wire);
                }
            }
        }
    }

    Ok(json!({
        "messages": wire_messages,
        "system": Value::Null,
    }))
}

fn render_text_and_images(blocks: &[ContentBlock]) -> Result<Value> {
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
            ContentBlock::Image(img) => {
                let url = match &img.source {
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    }
                    ImageSource::Url(u) => u.clone(),
                };
                arr.push(json!({
                    "type": "image_url",
                    "image_url": { "url": url }
                }));
            }
            ContentBlock::ToolCall(_) | ContentBlock::ToolResult(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "tool blocks must be serialized via `tool_calls` or role=tool messages"
                        .to_string(),
                })
            }
            ContentBlock::Document(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "native Ollama messages do not support document blocks".to_string(),
                })
            }
            ContentBlock::Reasoning(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: PROVIDER,
                    detail: "reasoning blocks must be serialized to `thinking` field".to_string(),
                })
            }
        }
    }
    Ok(Value::Array(arr))
}

fn looks_like_openai_message_shape(raw: &Value) -> bool {
    if raw.get("function").is_some() || raw.get("tool_call_id").is_some() {
        return true;
    }
    if let Some(tool_calls) = raw.get("tool_calls").and_then(Value::as_array) {
        return tool_calls.iter().any(|tc| tc.get("function").is_some());
    }
    false
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
