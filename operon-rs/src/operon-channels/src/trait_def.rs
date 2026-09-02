// trait_def.rs — Core channel abstractions and trait definitions for operon-channels.
//
// Hey friend! Welcome to the channel core module. This file defines the `Channel` trait
// and all common communication types used across Operon messaging integrations (WhatsApp,
// Telegram, etc.).
//
// DESIGN PRINCIPLE:
//   Every messaging platform (WhatsApp, Telegram, etc.) implements the `Channel` trait.
//   This allows Operon's frontends (GUI and TUI) to manage channels using a unified API
//   without caring about platform-specific protocol details under the hood.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// ─────────────────────────────────────────────────────────────────────────────
// Data Types
// ─────────────────────────────────────────────────────────────────────────────

/// Unique identifier for each messaging channel platform supported by Operon.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelId {
    /// WhatsApp multi-device connection.
    WhatsApp,
    /// Telegram bot / client connection.
    Telegram,
    /// Discord bot / Gateway connection.
    Discord,
    /// Slack bot / Socket Mode connection.
    Slack,
    /// Feishu / Lark bot / WebSocket connection.
    Feishu,
    /// Extensible custom or third-party channel name.
    Other(String),
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WhatsApp => write!(f, "whatsapp"),
            Self::Telegram => write!(f, "telegram"),
            Self::Discord => write!(f, "discord"),
            Self::Slack => write!(f, "slack"),
            Self::Feishu => write!(f, "feishu"),
            Self::Other(name) => write!(f, "{}", name),
        }
    }
}

/// Represents the current runtime connection state of a messaging channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelStatus {
    /// The channel service is stopped or offline.
    Disconnected,
    /// Network connection is actively being established.
    Connecting,
    /// Connection requires user authentication via QR code scanning.
    QrRequired(QrCodeState),
    /// Channel is active, paired, and ready to receive/send messages.
    Connected,
    /// Connection encountered an unrecoverable failure.
    Error(String),
}

/// State object holding QR code pairing data for GUI/TUI display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrCodeState {
    /// Raw QR code payload string (can be rendered as terminal ASCII or base64 SVG/PNG).
    pub payload: String,
    /// Unix timestamp in seconds when this QR code expires.
    pub expires_at: i64,
}

/// Generic normalized inbound or outbound channel message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMessage {
    /// Identifier of the channel this message belongs to.
    pub channel_id: ChannelId,
    /// Normalized sender contact identifier (e.g. phone number or telegram chat ID).
    pub sender_id: String,
    /// Text content of the message.
    pub text: String,
    /// Unix epoch timestamp (seconds) when the message was received or created.
    pub timestamp: i64,
    /// Whether the message sender is classified as an Owner (main number / allowlist) or External.
    pub is_owner: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Channel Trait
// ─────────────────────────────────────────────────────────────────────────────

/// The primary trait implemented by all messaging platform adapters (e.g. WhatsApp, Telegram).
#[async_trait]
pub trait Channel: Send + Sync {
    /// Returns the unique `ChannelId` for this platform adapter.
    fn id(&self) -> ChannelId;

    /// Starts the background channel engine and opens network listeners.
    async fn start(&self) -> Result<(), crate::ChannelError>;

    /// Stops the channel engine cleanly and closes sockets.
    async fn stop(&self) -> Result<(), crate::ChannelError>;

    /// Returns the current runtime connection status of the channel.
    async fn status(&self) -> ChannelStatus;

    /// Subscribes to QR code pairing updates emitted by the channel engine.
    ///
    /// Useful for GUI/TUI settings screens to render real-time QR codes during pairing.
    async fn subscribe_qr(&self) -> mpsc::Receiver<QrCodeState>;
}
