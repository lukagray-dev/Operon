// error.rs — Error definitions for operon-channels-discord.
//
// Hey friend! This module defines the `DiscordError` enum, wrapping connection issues,
// REST API failures, session execution errors, JSON parsing errors, and I/O failures.

use thiserror::Error;

/// Error types that can occur during Discord channel operations.
#[derive(Debug, Error)]
pub enum DiscordError {
    #[error("Discord bot token is not configured")]
    NotConfigured,

    #[error("Discord client is not connected")]
    NotConnected,

    #[error("Discord connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Discord message delivery failed: {0}")]
    SendFailed(String),

    #[error("Session execution error: {0}")]
    Session(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),
}

