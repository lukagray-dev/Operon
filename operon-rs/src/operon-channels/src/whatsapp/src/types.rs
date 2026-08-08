// types.rs — Domain types and structures for operon-channels-whatsapp.
//
// Hey friend! This file houses all core domain types for the WhatsApp channel integration.
// It includes contact number sanitization, message structures, and connection states.

use serde::{Deserialize, Serialize};

/// Normalized WhatsApp contact identifier (phone number).
///
/// Strips out non-digit characters (spaces, dashes, parentheses, plus signs)
/// to produce a clean, deterministic identifier used for folder names and session lookups.
///
/// Example: `+1 (555) 019-2834` -> `15550192834`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContactId(pub String);

impl ContactId {
    /// Create a new `ContactId` by sanitizing a raw input phone number string.
    pub fn new(raw: &str) -> Self {
        let clean: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        Self(clean)
    }

    /// Returns the sanitized string reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Represents an inbound message received over the WhatsApp WebSocket connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppMessage {
    /// Unique message identifier from WhatsApp.
    pub id: String,
    /// Contact ID of the sender.
    pub sender: ContactId,
    /// Raw text content of the message.
    pub text: String,
    /// Epoch timestamp (seconds) when received.
    pub timestamp: i64,
    /// Indicates whether the message originated from self (note-to-self / self-chat).
    pub is_self: bool,
}

/// Current status of the WhatsApp channel socket connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    QrRequired(QrCodeState),
    /// A pairing code has been issued by WhatsApp's servers and is ready
    /// for the user to enter in WhatsApp > Linked Devices.
    PairingCodeIssued(PairingCodeState),
    Connected,
    Error(String),
}

/// QR Code state information used for frontend pairing display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QrCodeState {
    /// Raw QR code string payload.
    pub payload: String,
    /// Expiry timestamp in seconds.
    pub expires_at: i64,
}

/// Pairing code state received from WhatsApp servers during pair-code linking.
///
/// The code is formatted as `XXXX-XXXX` (8 alphanumeric characters with a dash separator).
/// The user must enter this code in their WhatsApp mobile app under Linked Devices
/// within the expiry window (typically ~160 seconds from issuance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingCodeState {
    /// The real pairing code from WhatsApp, formatted as XXXX-XXXX.
    pub code: String,
    /// Unix timestamp (seconds) when this code expires — typically now + 160s.
    pub expires_at: i64,
}
