//! Public entry points for message normalization and denormalization.
//!
//! This module exposes the two crate-level conversion functions:
//! - [`normalize_message`]: provider wire JSON -> canonical message
//! - [`denormalize_messages`]: canonical messages -> provider wire JSON bundle

use serde_json::Value;

use crate::error::MessageNormalizeError;
use crate::provider::{FromWireMessage, Provider, ToWireMessages};
use crate::types::{ConversationMessage, ContentBlock, MessageRole};

/// Normalize one provider wire message payload into canonical
/// [`ConversationMessage`].
pub fn normalize_message(
    raw: Value,
    provider: &Provider,
) -> Result<ConversationMessage, MessageNormalizeError> {
    ConversationMessage::from_wire(raw, provider)
}

/// Denormalize canonical message history into provider wire JSON.
///
/// Returns an object with at least:
/// - `"messages"`: provider message array
/// - `"system"`: provider-level system string when relevant (or null)
pub fn denormalize_messages(
    msgs: &[ConversationMessage],
    provider: &Provider,
) -> Result<Value, MessageNormalizeError> {
    let mut mapped_msgs = Vec::with_capacity(msgs.len());

    for msg in msgs {
        let mapped_role = if msg.role == MessageRole::Tool {
            MessageRole::User
        } else {
            msg.role.clone()
        };

        let mut mapped_content = Vec::with_capacity(msg.content.len());

        for block in &msg.content {
            match block {
                ContentBlock::ToolCall(tc) => {
                    let tag = serialize_tool_call_to_tag(tc);
                    mapped_content.push(ContentBlock::Text(tag));
                }
                ContentBlock::ToolResult(tr) => {
                    let tag = serialize_tool_result_to_tag(tr);
                    mapped_content.push(ContentBlock::Text(tag));
                }
                other => mapped_content.push(other.clone()),
            }
        }

        mapped_msgs.push(ConversationMessage {
            role: mapped_role,
            content: mapped_content,
            stop_reason: msg.stop_reason.clone(),
        });
    }

    mapped_msgs.to_wire(provider)
}

fn serialize_tool_call_to_tag(tc: &crate::ToolCall) -> String {
    let mut parts = vec![format!("<{}", tc.name)];
    let mut body = None;

    if let Some(obj) = tc.arguments.as_object() {
        for (k, v) in obj {
            if k == "__body__" {
                body = v.as_str().map(|s| s.to_string());
            } else if let Some(s) = v.as_str() {
                parts.push(format!("{}=\"{}\"", k, s.replace("\"", "\\\"")));
            } else {
                parts.push(format!("{}=\"{}\"", k, v.to_string().replace("\"", "\\\"")));
            }
        }
    }

    let tag_header = parts.join(" ");

    if let Some(body_text) = body {
        format!("{}>\n<<<<\n{}\n>>>>", tag_header, body_text)
    } else {
        format!("{}>", tag_header)
    }
}

fn serialize_tool_result_to_tag(tr: &crate::ToolResult) -> String {
    let content_str = match &tr.content {
        crate::ToolContent::Text(text) => text.clone(),
    };

    format!(
        "<tool_result name=\"{}\" is_error=\"{}\">\n<<<<\n{}\n>>>>",
        tr.name, tr.is_error, content_str
    )
}
