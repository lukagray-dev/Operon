//! OpenAI-compatible message normalization and denormalization.
//!
//! This module implements the shared OpenAI family logic used by:
//! - OpenAI
//! - Groq
//! - Mistral
//! - DeepSeek (with extra `reasoning_content`)
//! - xAI (with extra `reasoning_content`)
//! - Ollama `/v1/chat/completions`
//! - OpenRouter (when OpenAI shape is detected)

use operon_context_normalize_reasoning::{
    denormalize_reasoning, normalize_reasoning, Provider as ReasoningProvider,
};
use operon_context_normalize_tools::{
    denormalize_result as denormalize_tool_result, normalize as normalize_tool_call,
    Provider as ToolProvider, ToolCall, ToolCallId, ToolContent, ToolResult,
};
use serde_json::{json, Value};

use crate::error::{MessageNormalizeError, Result};
use crate::stop_reason::normalize_stop_reason;
use crate::types::{ContentBlock, ConversationMessage, ImageBlock, ImageSource, MessageRole};

const PROVIDER: &str = "OpenAI";

/// Normalize an OpenAI wire message payload into canonical form.
pub fn normalize_message(raw: Value) -> Result<ConversationMessage> {
    normalize_message_with_provider(raw, PROVIDER)
}

/// Normalize an OpenAI-compatible wire message payload using a custom provider
/// name for error diagnostics.
pub fn normalize_message_with_provider(
    raw: Value,
    provider_name: &'static str,
) -> Result<ConversationMessage> {
    normalize_message_with_provider_and_reasoning(raw, provider_name, None, None)
}

/// Normalize an OpenAI-compatible payload with optional side-channel reasoning
/// field (used by DeepSeek/xAI).
pub fn normalize_message_with_provider_and_reasoning(
    raw: Value,
    provider_name: &'static str,
    reasoning_field: Option<&'static str>,
    reasoning_provider: Option<ReasoningProvider>,
) -> Result<ConversationMessage> {
    let (message_value, finish_reason) = extract_message_and_finish_reason(raw, provider_name)?;

    let role_str = message_value.get("role").and_then(Value::as_str).ok_or(
        MessageNormalizeError::MissingField {
            field: "role",
            provider: provider_name,
        },
    )?;

    let role = parse_openai_role(role_str, provider_name)?;

    if role == MessageRole::Tool {
        return normalize_tool_role_message(message_value, provider_name);
    }

    let mut content = parse_content_field(
        message_value.get("content"),
        provider_name,
        matches!(role, MessageRole::Assistant),
    )?;

    if matches!(role, MessageRole::Assistant) {
        // If this provider exposes a dedicated reasoning field, prepend those
        // reasoning blocks before normal answer text as requested.
        if let (Some(field), Some(rp)) = (reasoning_field, reasoning_provider) {
            if let Some(reasoning_raw) = message_value.get(field).cloned() {
                let mut reasoning_blocks = normalize_reasoning(reasoning_raw, &rp)
                    .map_err(|e| map_reasoning_err(e, provider_name))?;
                let mut canonical = Vec::with_capacity(reasoning_blocks.len() + content.len());
                for rb in reasoning_blocks.drain(..) {
                    canonical.push(ContentBlock::Reasoning(rb));
                }
                canonical.extend(content);
                content = canonical;
            }
        }

        if let Some(tool_calls) = message_value.get("tool_calls").and_then(Value::as_array) {
            for tc in tool_calls {
                let tool_call =
                    normalize_tool_call(tc.clone(), &tool_provider_from_name(provider_name))
                        .map_err(|e| map_tool_err(e, provider_name))?;
                content.push(ContentBlock::ToolCall(tool_call));
            }
        }
    }

    let mut out = ConversationMessage {
        role,
        content,
        stop_reason: None,
    };

    if let Some(raw_reason) = finish_reason {
        out.stop_reason = Some(normalize_stop_reason(
            &raw_reason,
            &provider_from_name(provider_name),
        ));
    }

    Ok(out)
}

/// Denormalize canonical messages into OpenAI wire message bundle.
pub fn denormalize_messages(msgs: &[ConversationMessage]) -> Result<Value> {
    denormalize_messages_with_provider(msgs, PROVIDER)
}

/// Denormalize canonical messages into OpenAI-compatible wire message bundle
/// using a custom provider name for error diagnostics.
pub fn denormalize_messages_with_provider(
    msgs: &[ConversationMessage],
    provider_name: &'static str,
) -> Result<Value> {
    denormalize_messages_with_provider_and_reasoning(msgs, provider_name, None, None)
}

/// Denormalize canonical messages into OpenAI-compatible bundle with optional
/// side-channel reasoning field (DeepSeek/xAI).
pub fn denormalize_messages_with_provider_and_reasoning(
    msgs: &[ConversationMessage],
    provider_name: &'static str,
    reasoning_field: Option<&'static str>,
    reasoning_provider: Option<ReasoningProvider>,
) -> Result<Value> {
    let mut wire_messages: Vec<Value> = Vec::new();

    for msg in msgs {
        match msg.role {
            MessageRole::System => {
                let system_text = render_system_content_as_string(&msg.content, provider_name)?;
                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String("system".to_string()));
                obj.insert("content".to_string(), Value::String(system_text));
                wire_messages.push(Value::Object(obj));
            }
            MessageRole::User => {
                let content_value =
                    render_openai_text_image_content(&msg.content, provider_name, true)?;
                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String("user".to_string()));
                obj.insert("content".to_string(), content_value);
                
                // NOTE: We do not serialize stop_reason / finish_reason back into the messages list
                // for the API request payload because providers only accept those fields in model outputs,
                // and passing them in input messages leads to HTTP 400 Bad Request.
                wire_messages.push(Value::Object(obj));
            }
            MessageRole::Assistant => {
                let mut plain_blocks: Vec<ContentBlock> = Vec::new();
                let mut reasoning_blocks = Vec::new();
                let mut tool_calls: Vec<ToolCall> = Vec::new();

                for block in &msg.content {
                    match block {
                        ContentBlock::ToolCall(tc) => tool_calls.push(tc.clone()),
                        ContentBlock::Reasoning(rb) => reasoning_blocks.push(rb.clone()),
                        other => plain_blocks.push(other.clone()),
                    }
                }

                let content_value = if plain_blocks.is_empty() && !tool_calls.is_empty() {
                    Value::Null
                } else {
                    render_openai_text_image_content(&plain_blocks, provider_name, false)?
                };

                let mut obj = serde_json::Map::new();
                obj.insert("role".to_string(), Value::String("assistant".to_string()));
                obj.insert("content".to_string(), content_value);

                if !tool_calls.is_empty() {
                    let mut calls = Vec::with_capacity(tool_calls.len());
                    for tc in tool_calls {
                        calls.push(tool_call_to_openai_wire(&tc));
                    }
                    obj.insert("tool_calls".to_string(), Value::Array(calls));
                }

                if let (Some(field), Some(rp)) = (reasoning_field, reasoning_provider.clone()) {
                    if !reasoning_blocks.is_empty() {
                        let raw = denormalize_reasoning(&reasoning_blocks, &rp)
                            .map_err(|e| map_reasoning_err(e, provider_name))?;
                        obj.insert(field.to_string(), raw);
                    }
                } else if !reasoning_blocks.is_empty() {
                    return Err(MessageNormalizeError::UnsupportedContentType {
                        provider: provider_name,
                        detail: "assistant reasoning blocks are not supported in OpenAI wire content without a provider-specific reasoning field".to_string(),
                    });
                }

                // NOTE: We do not serialize stop_reason / finish_reason back into the messages list
                // for the API request payload because providers only accept those fields in model outputs,
                // and passing them in input messages leads to HTTP 400 Bad Request.
                wire_messages.push(Value::Object(obj));
            }
            MessageRole::Tool => {
                let tool_results = extract_tool_results(&msg.content, provider_name)?;
                for tr in tool_results {
                    let wire = denormalize_tool_result(tr, &tool_provider_from_name(provider_name))
                        .map_err(|e| map_tool_err(e, provider_name))?;
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

fn extract_message_and_finish_reason(
    raw: Value,
    provider_name: &'static str,
) -> Result<(Value, Option<String>)> {
    if let Some(choices) = raw.get("choices").and_then(Value::as_array) {
        let first = choices.first().ok_or(MessageNormalizeError::MissingField {
            field: "choices[0]",
            provider: provider_name,
        })?;

        let message = first
            .get("message")
            .ok_or(MessageNormalizeError::MissingField {
                field: "choices[0].message",
                provider: provider_name,
            })?
            .clone();

        let finish_reason = first
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string);

        return Ok((message, finish_reason));
    }

    let finish_reason = raw
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok((raw, finish_reason))
}

fn parse_openai_role(raw: &str, provider_name: &'static str) -> Result<MessageRole> {
    match raw {
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        "system" => Ok(MessageRole::System),
        "tool" => Ok(MessageRole::Tool),
        other => Err(MessageNormalizeError::UnknownRole {
            role: other.to_string(),
            provider: provider_name,
        }),
    }
}

fn parse_content_field(
    raw_content: Option<&Value>,
    provider_name: &'static str,
    is_assistant: bool,
) -> Result<Vec<ContentBlock>> {
    match raw_content {
        None | Some(Value::Null) => {
            if is_assistant {
                Ok(Vec::new())
            } else {
                Ok(vec![ContentBlock::Text(String::new())])
            }
        }
        Some(Value::String(s)) => Ok(vec![ContentBlock::Text(s.to_string())]),
        Some(Value::Array(arr)) => parse_openai_content_array(arr, provider_name),
        Some(other) => Err(MessageNormalizeError::UnknownShape {
            provider: provider_name,
            detail: format!("expected `content` as string, array, or null, found: {other}"),
        }),
    }
}

fn parse_openai_content_array(
    arr: &[Value],
    provider_name: &'static str,
) -> Result<Vec<ContentBlock>> {
    let mut out = Vec::new();
    for block in arr {
        let t = block.get("type").and_then(Value::as_str).ok_or(
            MessageNormalizeError::MissingField {
                field: "content[].type",
                provider: provider_name,
            },
        )?;

        match t {
            "text" | "input_text" | "output_text" => {
                let text = block.get("text").and_then(Value::as_str).ok_or(
                    MessageNormalizeError::MissingField {
                        field: "content[].text",
                        provider: provider_name,
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
                        provider: provider_name,
                    })?;
                out.push(ContentBlock::Image(ImageBlock {
                    source: parse_image_source_from_url(url),
                }));
            }
            other => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: format!("unsupported OpenAI content block type `{other}`"),
                })
            }
        }
    }
    Ok(out)
}

fn parse_image_source_from_url(url: &str) -> ImageSource {
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

fn normalize_tool_role_message(
    message_value: Value,
    provider_name: &'static str,
) -> Result<ConversationMessage> {
    let call_id = message_value
        .get("tool_call_id")
        .and_then(Value::as_str)
        .ok_or(MessageNormalizeError::MissingField {
            field: "tool_call_id",
            provider: provider_name,
        })?;

    let name = message_value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let content_value = message_value.get("content").cloned().unwrap_or(Value::Null);
    let content = parse_tool_content_from_wire(content_value);
    let is_error = message_value
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let tool_result = ToolResult {
        call_id: ToolCallId(call_id.to_string()),
        name,
        content,
        is_error,
    };

    Ok(ConversationMessage {
        role: MessageRole::Tool,
        content: vec![ContentBlock::ToolResult(tool_result)],
        stop_reason: None,
    })
}

fn parse_tool_content_from_wire(raw: Value) -> ToolContent {
    match raw {
        Value::Null => ToolContent::Text(String::new()),
        Value::String(s) => ToolContent::Text(s),
        Value::Array(arr) => {
            let mut text_parts = Vec::new();
            for item in &arr {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(s) = item.get("text").and_then(Value::as_str) {
                        text_parts.push(s.to_string());
                    }
                }
            }
            if !text_parts.is_empty() {
                ToolContent::Text(text_parts.join("\n"))
            } else {
                ToolContent::Json(Value::Array(arr))
            }
        }
        other => ToolContent::Json(other),
    }
}

fn render_openai_text_image_content(
    blocks: &[ContentBlock],
    provider_name: &'static str,
    allow_empty_string: bool,
) -> Result<Value> {
    let mut typed_blocks: Vec<Value> = Vec::new();
    let mut only_text: Vec<String> = Vec::new();
    let mut has_non_text = false;

    for block in blocks {
        match block {
            ContentBlock::Text(s) => {
                typed_blocks.push(json!({"type": "text", "text": s}));
                only_text.push(s.clone());
            }
            ContentBlock::Image(img) => {
                has_non_text = true;
                let url = match &img.source {
                    ImageSource::Base64 { media_type, data } => {
                        format!("data:{media_type};base64,{data}")
                    }
                    ImageSource::Url(url) => url.clone(),
                };
                typed_blocks.push(json!({
                    "type": "image_url",
                    "image_url": { "url": url }
                }));
            }
            ContentBlock::ToolCall(_) | ContentBlock::ToolResult(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: "tool blocks must be represented in `tool_calls` or `role=tool` messages for OpenAI-compatible formats".to_string(),
                })
            }
            ContentBlock::Document(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: "document blocks are not supported by OpenAI-compatible message content".to_string(),
                })
            }
            ContentBlock::Reasoning(_) => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: "reasoning blocks require a provider-specific reasoning field".to_string(),
                })
            }
        }
    }

    if typed_blocks.is_empty() {
        if allow_empty_string {
            return Ok(Value::String(String::new()));
        }
        return Ok(Value::Array(Vec::new()));
    }

    if !has_non_text && typed_blocks.len() == 1 {
        return Ok(Value::String(only_text.join("")));
    }

    Ok(Value::Array(typed_blocks))
}

fn render_system_content_as_string(
    content: &[ContentBlock],
    provider_name: &'static str,
) -> Result<String> {
    let mut text = Vec::new();
    for block in content {
        match block {
            ContentBlock::Text(s) => text.push(s.clone()),
            _ => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: "system messages can only contain text blocks".to_string(),
                })
            }
        }
    }
    Ok(text.join("\n\n"))
}

fn extract_tool_results<'a>(
    content: &'a [ContentBlock],
    provider_name: &'static str,
) -> Result<Vec<&'a ToolResult>> {
    let mut out = Vec::new();
    for block in content {
        match block {
            ContentBlock::ToolResult(tr) => out.push(tr),
            _ => {
                return Err(MessageNormalizeError::UnsupportedContentType {
                    provider: provider_name,
                    detail: "role=tool messages may only contain ToolResult blocks".to_string(),
                })
            }
        }
    }
    if out.is_empty() {
        return Err(MessageNormalizeError::MissingField {
            field: "tool_result",
            provider: provider_name,
        });
    }
    Ok(out)
}

fn tool_call_to_openai_wire(call: &ToolCall) -> Value {
    json!({
        "id": call.id.0,
        "type": "function",
        "function": {
            "name": call.name,
            "arguments": call.arguments.to_string(),
        }
    })
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

fn tool_provider_from_name(provider_name: &'static str) -> ToolProvider {
    match provider_name {
        "Anthropic" => ToolProvider::Anthropic,
        "OpenAI" => ToolProvider::OpenAI,
        "Gemini" => ToolProvider::Gemini,
        "Ollama" => ToolProvider::Ollama,
        "DeepSeek" => ToolProvider::DeepSeek,
        "OpenRouter" => ToolProvider::OpenRouter,
        "Groq" => ToolProvider::Groq,
        "Mistral" => ToolProvider::Mistral,
        "xAI" => ToolProvider::XAI,
        "NVIDIA NIM" => ToolProvider::NvidiaNim,
        "Cohere" => ToolProvider::Cohere,
        _ => ToolProvider::OpenAI,
    }
}

fn provider_from_name(provider_name: &'static str) -> crate::provider::Provider {
    match provider_name {
        "Anthropic" => crate::provider::Provider::Anthropic,
        "OpenAI" => crate::provider::Provider::OpenAI,
        "Gemini" => crate::provider::Provider::Gemini,
        "Ollama" => crate::provider::Provider::Ollama,
        "DeepSeek" => crate::provider::Provider::DeepSeek,
        "OpenRouter" => crate::provider::Provider::OpenRouter,
        "Groq" => crate::provider::Provider::Groq,
        "Mistral" => crate::provider::Provider::Mistral,
        "xAI" => crate::provider::Provider::XAI,
        "NVIDIA NIM" => crate::provider::Provider::NvidiaNim,
        "Cohere" => crate::provider::Provider::Cohere,
        _ => crate::provider::Provider::OpenAI,
    }
}
