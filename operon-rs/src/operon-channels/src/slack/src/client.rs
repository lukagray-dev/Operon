// client.rs — Slack Web API and Socket Mode WebSocket client.
//
// Hey friend! This module handles direct interaction with Slack:
// 1. Validates tokens via `auth.test`.
// 2. Opens a Socket Mode WebSocket tunnel via `apps.connections.open`.
// 3. Receives live message events, instantly ACKing every envelope with its `envelope_id`.
// 4. Sends outbound responses via `chat.postMessage`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, error, info, warn};

use crate::config::SlackConfig;
use crate::error::SlackError;
use crate::types::{ConnectionStatus, SlackChannelId, SlackMessage, SocketModeEnvelope, UserId};

/// Slack REST API base endpoint.
pub const SLACK_API_BASE: &str = "https://slack.com/api";

#[derive(Debug, Deserialize)]
struct AuthTestResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AppsConnectionsOpenResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostMessagePayload<'a> {
    channel: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct PostMessageResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    ts: Option<String>,
}

#[derive(Debug, Serialize)]
struct SocketModeAckPayload<'a> {
    envelope_id: &'a str,
}

/// Direct client for Slack REST API and Socket Mode WebSocket connection.
pub struct SlackClient {
    config: SlackConfig,
    http_client: reqwest::Client,
    status: Arc<AsyncMutex<ConnectionStatus>>,
    is_running: Arc<AtomicBool>,
    inbound_rx: Arc<AsyncMutex<Option<mpsc::Receiver<SlackMessage>>>>,
    inbound_tx: mpsc::Sender<SlackMessage>,
    bot_user_id: Arc<AsyncMutex<Option<String>>>,
}

impl SlackClient {
    /// Creates a new `SlackClient` with the given configuration.
    pub fn new(config: SlackConfig) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel::<SlackMessage>(128);
        Self {
            config,
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            status: Arc::new(AsyncMutex::new(ConnectionStatus::Disconnected)),
            is_running: Arc::new(AtomicBool::new(false)),
            inbound_rx: Arc::new(AsyncMutex::new(Some(inbound_rx))),
            inbound_tx,
            bot_user_id: Arc::new(AsyncMutex::new(None)),
        }
    }

    /// Returns the current runtime connection status.
    pub async fn status(&self) -> ConnectionStatus {
        self.status.lock().await.clone()
    }

    /// Returns whether the client worker is currently running.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Takes the inbound message receiver (can only be called once).
    pub async fn take_message_receiver(&self) -> Option<mpsc::Receiver<SlackMessage>> {
        self.inbound_rx.lock().await.take()
    }

    /// Validates the Bot Token by calling `auth.test`.
    pub async fn test_auth(&self) -> Result<String, SlackError> {
        let bot_token = self.config.bot_token.as_deref().ok_or(SlackError::NotConfigured)?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", bot_token))
                .map_err(|e| SlackError::ConnectionFailed(e.to_string()))?,
        );

        let resp = self
            .http_client
            .post(format!("{}/auth.test", SLACK_API_BASE))
            .headers(headers)
            .send()
            .await?;

        let auth_res: AuthTestResponse = resp.json().await?;
        if !auth_res.ok {
            let err_msg = auth_res
                .error
                .unwrap_or_else(|| "Unknown auth error".to_string());
            return Err(SlackError::ConnectionFailed(err_msg));
        }

        let user = auth_res.user.unwrap_or_default();
        let team = auth_res.team.unwrap_or_default();
        let user_id = auth_res.user_id.unwrap_or_default();

        if let Ok(mut lock) = self.bot_user_id.try_lock() {
            *lock = Some(user_id.clone());
        }

        Ok(format!("Authenticated as @{} ({}) on team {}", user, user_id, team))
    }

    /// Requests a new Socket Mode WebSocket URL using the App-Level Token (`xapp-...`).
    async fn request_socket_mode_url(&self) -> Result<String, SlackError> {
        let app_token = self.config.app_token.as_deref().ok_or_else(|| {
            SlackError::ConnectionFailed(
                "No app_token (xapp-...) provided for Slack Socket Mode".to_string(),
            )
        })?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", app_token))
                .map_err(|e| SlackError::ConnectionFailed(e.to_string()))?,
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/x-www-form-urlencoded"),
        );

        let resp = self
            .http_client
            .post(format!("{}/apps.connections.open", SLACK_API_BASE))
            .headers(headers)
            .send()
            .await?;

        let conn_res: AppsConnectionsOpenResponse = resp.json().await?;
        if !conn_res.ok {
            let err_msg = conn_res
                .error
                .unwrap_or_else(|| "Failed to open socket connection".to_string());
            return Err(SlackError::ConnectionFailed(err_msg));
        }

        conn_res.url.ok_or_else(|| {
            SlackError::ConnectionFailed("Slack returned ok:true without a WebSocket URL".to_string())
        })
    }

    /// Connects to Slack via Socket Mode WebSocket and begins ingesting events.
    pub async fn connect(&self) -> Result<(), SlackError> {
        // 1. Test bot credentials and cache bot user ID
        let auth_info = self.test_auth().await?;
        info!("Slack authentication verified: {}", auth_info);

        // 2. Obtain Socket Mode WebSocket endpoint
        let ws_url = self.request_socket_mode_url().await?;
        info!("Slack Socket Mode WebSocket URL obtained");

        {
            let mut status = self.status.lock().await;
            *status = ConnectionStatus::Connecting;
        }

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .map_err(|e| SlackError::ConnectionFailed(e.to_string()))?;

        {
            let mut status = self.status.lock().await;
            *status = ConnectionStatus::Connected;
        }
        self.is_running.store(true, Ordering::SeqCst);

        let (mut write_half, mut read_half) = ws_stream.split();
        let inbound_tx = self.inbound_tx.clone();
        let status = self.status.clone();
        let is_running = self.is_running.clone();
        let bot_user_id = self.bot_user_id.lock().await.clone();

        // Spawn WebSocket ingestion and ACK loop
        tokio::spawn(async move {
            info!("Slack Socket Mode event listener loop started");

            while let Some(msg_result) = read_half.next().await {
                match msg_result {
                    Ok(WsMessage::Text(text)) => {
                        debug!("Received Socket Mode frame: {}", text);

                        if let Ok(envelope) = serde_json::from_str::<SocketModeEnvelope>(&text) {
                            // 1. Immediately acknowledge envelope
                            let ack_body = SocketModeAckPayload {
                                envelope_id: &envelope.envelope_id,
                            };
                            if let Ok(ack_json) = serde_json::to_string(&ack_body) {
                                if let Err(e) = write_half.send(WsMessage::Text(ack_json.into())).await {
                                    warn!("Failed to send Socket Mode ACK frame: {}", e);
                                }
                            }

                            // 2. Handle events_api payload
                            if envelope.payload_type == "events_api" {
                                if let Some(event) = envelope.payload.get("event") {
                                    let event_type = event
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or_default();

                                    if event_type == "message" {
                                        let subtype = event.get("subtype").and_then(|v| v.as_str());
                                        let bot_id = event.get("bot_id").and_then(|v| v.as_str());
                                        let user = event
                                            .get("user")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default();
                                        let text_content = event
                                            .get("text")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default();
                                        let channel = event
                                            .get("channel")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default();
                                        let ts = event
                                            .get("ts")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default();
                                        let thread_ts = event
                                            .get("thread_ts")
                                            .and_then(|v| v.as_str())
                                            .map(String::from);

                                        // Ignore bot messages, message updates, and our own messages
                                        let is_bot = bot_id.is_some() || subtype == Some("bot_message");
                                        let is_self = bot_user_id
                                            .as_ref()
                                            .map(|b| b == user)
                                            .unwrap_or(false);

                                        if !is_bot && !is_self && subtype.is_none() && !user.is_empty() && !channel.is_empty() {
                                            let now_secs = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .map(|d| d.as_secs() as i64)
                                                .unwrap_or(0);

                                            let slack_msg = SlackMessage {
                                                id: ts.to_string(),
                                                channel_id: SlackChannelId::new(channel),
                                                author_id: UserId::new(user),
                                                text: text_content.to_string(),
                                                thread_ts,
                                                timestamp: now_secs,
                                                is_bot: false,
                                            };

                                            info!(
                                                user = %slack_msg.author_id,
                                                channel = %slack_msg.channel_id,
                                                "Forwarding inbound Slack message to router"
                                            );

                                            if let Err(e) = inbound_tx.send(slack_msg).await {
                                                error!("Failed to enqueue inbound Slack message: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(WsMessage::Ping(payload)) => {
                        let _ = write_half.send(WsMessage::Pong(payload)).await;
                    }
                    Ok(WsMessage::Close(frame)) => {
                        info!("Slack Socket Mode closed by server: {:?}", frame);
                        break;
                    }
                    Err(e) => {
                        warn!("Slack Socket Mode WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            {
                let mut st = status.lock().await;
                *st = ConnectionStatus::Disconnected;
            }
            is_running.store(false, Ordering::SeqCst);
            info!("Slack Socket Mode WebSocket reader loop exited");
        });

        Ok(())
    }

    /// Sends a message to a Slack channel or thread using `chat.postMessage`.
    pub async fn send_message(
        &self,
        channel_id: &SlackChannelId,
        text: &str,
        thread_ts: Option<&str>,
    ) -> Result<String, SlackError> {
        let bot_token = self.config.bot_token.as_deref().ok_or(SlackError::NotConfigured)?;

        let payload = PostMessagePayload {
            channel: channel_id.as_str(),
            text,
            thread_ts,
        };

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", bot_token))
                .map_err(|e| SlackError::SendFailed(e.to_string()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let resp = self
            .http_client
            .post(format!("{}/chat.postMessage", SLACK_API_BASE))
            .headers(headers)
            .json(&payload)
            .send()
            .await?;

        let post_res: PostMessageResponse = resp.json().await?;
        if !post_res.ok {
            let err_msg = post_res
                .error
                .unwrap_or_else(|| "Failed to post message".to_string());
            return Err(SlackError::SendFailed(err_msg));
        }

        Ok(post_res.ts.unwrap_or_default())
    }
}

