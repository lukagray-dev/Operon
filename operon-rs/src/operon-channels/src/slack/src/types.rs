// types.rs — Core domain types for operon-channels-slack.
//
// Hey friend! Welcome to the Slack channel types module. Here we define
// strongly-typed identifiers for Slack users, channels, inbound messages,
// and Socket Mode envelope structures.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Strongly-typed identifier for a Slack User (e.g. "U0123456789" or "W0123456789").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl UserId {
    /// Creates a new `UserId` after trimming and stripping mention syntax `<@U...>`.
    pub fn new(id: impl Into<String>) -> Self {
        let raw = id.into();
        let trimmed = raw.trim();
        let cleaned = if (trimmed.starts_with("<@") || trimmed.starts_with("<@!"))
            && trimmed.ends_with('>')
        {
            let inner = trimmed
                .trim_start_matches("<@!")
                .trim_start_matches("<@")
                .trim_end_matches('>');
            inner.trim()
        } else {
            trimmed
        };
        Self(cleaned.to_string())
    }

    /// Returns the raw user identifier string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for a Slack Channel (e.g. "C0123456789" or "D0123456789").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackChannelId(String);

impl SlackChannelId {
    /// Creates a new `SlackChannelId` after stripping `<#C...|name>` or `<#C...>` syntax.
    pub fn new(id: impl Into<String>) -> Self {
        let raw = id.into();
        let trimmed = raw.trim();
        let cleaned = if trimmed.starts_with("<#") && trimmed.ends_with('>') {
            let inner = trimmed.trim_start_matches("<#").trim_end_matches('>');
            if let Some((ch_id, _name)) = inner.split_once('|') {
                ch_id.trim()
            } else {
                inner.trim()
            }
        } else {
            trimmed
        };
        Self(cleaned.to_string())
    }

    /// Returns the raw channel identifier string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SlackChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Normalized inbound message received from Slack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    /// Slack message timestamp / ID (e.g. "1672531199.000100").
    pub id: String,
    /// Channel where the message was posted.
    pub channel_id: SlackChannelId,
    /// Author user ID who sent the message.
    pub author_id: UserId,
    /// Plain text body of the message.
    pub text: String,
    /// Optional thread timestamp if this message is inside a thread.
    pub thread_ts: Option<String>,
    /// Unix timestamp (seconds) when the message was received.
    pub timestamp: i64,
    /// Whether this message was sent by a bot.
    pub is_bot: bool,
}

/// Connection status of the Slack Socket Mode client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Disconnected from Slack Socket Mode.
    Disconnected,
    /// Socket Mode WebSocket connection is currently opening.
    Connecting,
    /// Connected and actively receiving events over WebSocket.
    Connected,
    /// Encountered an unrecoverable error.
    Error(String),
}

/// Socket Mode payload envelope sent by Slack over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketModeEnvelope {
    /// Unique identifier for this envelope that must be ACKed.
    pub envelope_id: String,
    /// Type of the payload (e.g. "events_api", "slash_commands", "interactive").
    #[serde(rename = "type")]
    pub payload_type: String,
    /// Inner payload data containing the actual event.
    #[serde(default)]
    pub payload: serde_json::Value,
    /// Whether Slack accepts a response payload in the ACK.
    #[serde(default)]
    pub accepts_response_payload: bool,
}

