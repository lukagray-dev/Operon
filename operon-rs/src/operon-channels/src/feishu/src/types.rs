// types.rs — Domain types and message primitives for Feishu / Lark channel.
//
// Hey friend! This module defines the core data types for Feishu / Lark:
// 1. `UserId`: Strongly typed Feishu user identifier (typically Open ID `ou_...`).
// 2. `ChatId`: Feishu chat/group identifier (`oc_...`).
// 3. `FeishuDomain`: Target API environment (Feishu vs Lark).
// 4. `FeishuMessage`: Normalized inbound message representation.
// 5. `ConnectionStatus`: Channel connection state.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Target deployment platform / region for Feishu / Lark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeishuDomain {
    /// Feishu (Mainland China endpoint: `https://open.feishu.cn`)
    Feishu,
    /// Lark (International endpoint: `https://open.larksuite.com`)
    Lark,
}

impl Default for FeishuDomain {
    fn default() -> Self {
        Self::Feishu
    }
}

impl FeishuDomain {
    /// Returns the REST API base URL for this domain.
    pub fn api_base_url(&self) -> &'static str {
        match self {
            Self::Feishu => "https://open.feishu.cn/open-apis",
            Self::Lark => "https://open.larksuite.com/open-apis",
        }
    }

    /// Returns the WebSocket persistent connection endpoint URL for this domain.
    pub fn websocket_url(&self) -> &'static str {
        match self {
            Self::Feishu => "wss://ws-open.feishu.cn/ws/v2",
            Self::Lark => "wss://ws-open.larksuite.com/ws/v2",
        }
    }
}

impl fmt::Display for FeishuDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Feishu => write!(f, "feishu"),
            Self::Lark => write!(f, "lark"),
        }
    }
}

/// Strongly typed Feishu user identifier (typically an Open ID `ou_...` or user_id).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl UserId {
    /// Creates a new `UserId` after sanitizing enclosing brackets and mention tags.
    pub fn new(raw: impl Into<String>) -> Self {
        let s = raw.into().trim().to_string();
        let cleaned = s
            .trim_start_matches("<@")
            .trim_start_matches('@')
            .trim_end_matches('>')
            .trim()
            .to_string();
        Self(cleaned)
    }

    /// Returns the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly typed Feishu chat identifier (`oc_...`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatId(String);

impl ChatId {
    /// Creates a new `ChatId` after trimming whitespace.
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into().trim().to_string())
    }

    /// Returns the inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ChatId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Inbound message from a Feishu / Lark user or group chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    /// Feishu message ID (`om_...`).
    pub id: String,
    /// Chat ID (`oc_...`).
    pub chat_id: ChatId,
    /// Author open_id or user_id.
    pub author_id: UserId,
    /// Plaintext or extracted text content.
    pub text: String,
    /// Optional root message ID for threaded conversations.
    pub root_id: Option<String>,
    /// Optional parent message ID for replies.
    pub parent_id: Option<String>,
    /// Creation timestamp in milliseconds or seconds.
    pub timestamp: i64,
    /// Whether the message originated from a bot.
    pub is_bot: bool,
}

/// Connection status of the Feishu channel client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

impl Default for ConnectionStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

