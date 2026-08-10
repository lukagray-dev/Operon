// types.rs — Domain types and structures for operon-channels-telegram.
//
// Hey friend! This file houses all core domain types for the Telegram channel integration.
// It includes Telegram chat identifiers, message structures, and connection states.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Native Telegram chat identifier.
///
/// Telegram identifies chats (direct user chats, group chats, channels) using a stable 64-bit signed integer.
/// No string sanitization is needed (unlike WhatsApp phone numbers) because Telegram supplies stable `i64` IDs directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(pub i64);

impl ChatId {
    /// Create a new `ChatId` from a raw 64-bit integer.
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// Returns the underlying raw 64-bit integer value.
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an inbound message received over the Telegram Bot API long-polling loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramMessage {
    /// Unique update identifier from Telegram `getUpdates`.
    pub update_id: i64,
    /// Unique message identifier within the chat.
    pub message_id: i64,
    /// Chat ID of the sender/chat.
    pub sender: ChatId,
    /// Raw text or caption content of the message.
    pub text: String,
    /// Epoch timestamp (seconds) when received.
    pub timestamp: i64,
    /// Indicates whether the message originated from self. Always false for Telegram bots.
    pub is_self: bool,
}

/// Current status of the Telegram channel connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}
