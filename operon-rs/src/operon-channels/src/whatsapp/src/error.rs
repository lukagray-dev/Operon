// error.rs — Unified error types for operon-channels-whatsapp.
//
// Hey friend! This file defines `WhatsAppError`, covering all possible failure modes
// in authentication, socket connectivity, workspace management, and session execution.

#[derive(Debug, thiserror::Error)]
pub enum WhatsAppError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Socket connection closed: {0}")]
    SocketClosed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Invalid contact: {0}")]
    InvalidContact(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("QR generation failed: {0}")]
    QrGenerationFailed(String),

    #[error("Workspace error: {0}")]
    Workspace(String),

    #[error("Session runner error: {0}")]
    Session(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
