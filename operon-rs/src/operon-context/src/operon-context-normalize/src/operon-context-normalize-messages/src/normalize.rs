//! Public entry points for message normalization and denormalization.
//!
//! This module exposes the two crate-level conversion functions:
//! - [`normalize_message`]: provider wire JSON -> canonical message
//! - [`denormalize_messages`]: canonical messages -> provider wire JSON bundle

use serde_json::Value;

use crate::error::MessageNormalizeError;
use crate::provider::{FromWireMessage, Provider, ToWireMessages};
use crate::types::ConversationMessage;

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
    msgs.to_vec().to_wire(provider)
}
