// error.rs — Typed error definitions for operon-channels-slack.

use thiserror::Error;

/// Error types that can occur during Slack channel operations.
#[derive(Debug, Error)]
pub enum SlackError {
    #[error("Slack token is not configured")]
    NotConfigured,

    #[error("Slack client is not connected")]
    NotConnected,

    #[error("Slack connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Slack message delivery failed: {0}")]
    SendFailed(String),

    #[error("Session execution error: {0}")]
    Session(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}

