// error.rs — Typed error definitions for operon-channels-feishu.
//
// Hey friend! This module defines the `FeishuError` enum, wrapping connection issues,
// Open API REST failures, token expiration, session execution errors, and WebSocket errors.

use thiserror::Error;

/// Error types that can occur during Feishu / Lark channel operations.
#[derive(Debug, Error)]
pub enum FeishuError {
    #[error("Feishu App ID or App Secret is not configured")]
    NotConfigured,

    #[error("Feishu client is not connected")]
    NotConnected,

    #[error("Feishu connection or auth failed: {0}")]
    ConnectionFailed(String),

    #[error("Feishu message delivery failed: {0}")]
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

