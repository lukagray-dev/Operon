// error.rs — Unified error types for operon-channels-telegram.
//
// Hey friend! This file defines `TelegramError`, covering all possible failure modes
// in authentication, Bot API connectivity, workspace management, session execution, and JSON parsing.

#[derive(Debug, thiserror::Error)]
pub enum TelegramError {
    #[error("Missing Telegram bot token in configuration")]
    MissingBotToken,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Invalid chat: {0}")]
    InvalidChat(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

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

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}
