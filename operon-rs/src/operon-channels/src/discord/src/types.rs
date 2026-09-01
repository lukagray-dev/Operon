// types.rs — Domain types and structures for operon-channels-discord.
//
// Hey friend! This module contains domain types for the Discord channel integration.
// It includes user snowflake ID sanitization, channel IDs, normalized message models,
// and connection status enums used across the backend and frontend.

use serde::{Deserialize, Serialize};

/// Normalized Discord User Snowflake Identifier.
///
/// Strips non-digit characters to produce a deterministic, canonical user ID
/// string used for folder paths and session isolation lookups.
/// Example: `<@!123456789012345678>` -> `123456789012345678`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

impl UserId {
    /// Creates a new `UserId` by sanitizing raw snowflake or mention strings.
    pub fn new(raw: &str) -> Self {
        let clean: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        Self(clean)
    }

    /// Returns the sanitized user ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Normalized Discord Channel/Thread Snowflake Identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiscordChannelId(pub String);

impl DiscordChannelId {
    /// Creates a new `DiscordChannelId` by sanitizing raw channel ID strings.
    pub fn new(raw: &str) -> Self {
        let clean: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        Self(clean)
    }

    /// Returns the sanitized channel ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiscordChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an inbound message received from Discord Gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    /// Unique Discord message snowflake ID.
    pub id: String,
    /// Channel or thread snowflake ID where the message was sent.
    pub channel_id: DiscordChannelId,
    /// Author snowflake user ID.
    pub author_id: UserId,
    /// Author username / display tag (e.g. `lukagray`).
    pub author_username: String,
    /// Raw text content of the message.
    pub content: String,
    /// Unix epoch timestamp in seconds.
    pub timestamp: i64,
    /// Whether the message originated from the bot itself.
    pub is_self: bool,
    /// Whether the message author is flagged as a bot account.
    pub is_bot: bool,
}

/// Connection status of the Discord Gateway WebSocket and REST client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// The channel is offline or stopped.
    Disconnected,
    /// Actively establishing REST verification or Gateway WebSocket connection.
    Connecting,
    /// Gateway connection established, identified, and actively listening for messages.
    Connected,
    /// Connection encountered an error.
    Error(String),
}

