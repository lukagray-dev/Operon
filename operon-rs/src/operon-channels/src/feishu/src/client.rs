// client.rs — Feishu / Lark Open API and WebSocket Long Connection client.
//
// Hey friend! This module handles direct interaction with Feishu / Lark:
// 1. Manages internal `tenant_access_token` automatic acquisition and caching.
// 2. Tests bot credentials via `/bot/v3/info`.
// 3. Establishes a persistent WebSocket connection to stream live events without public webhooks.
// 4. Sends outbound replies via `/im/v1/messages`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, info, warn};

use crate::config::FeishuConfig;
use crate::error::FeishuError;
use crate::types::{ChatId, ConnectionStatus, FeishuMessage, UserId};

#[derive(Debug, Serialize)]
struct TenantAccessTokenReq<'a> {
    app_id: &'a str,
    app_secret: &'a str,
}

#[derive(Debug, Deserialize)]
struct TenantAccessTokenResp {
    code: i64,
    msg: String,
    #[serde(default)]
    tenant_access_token: Option<String>,
    #[serde(default)]
    expire: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BotInfoResp {
    code: i64,
    msg: String,
    #[serde(default)]
    bot: Option<BotInfoData>,
}

#[derive(Debug, Deserialize)]
struct BotInfoData {
    #[serde(default)]
    app_name: String,
    #[serde(default)]
    open_id: String,
}

#[derive(Debug, Deserialize)]
struct SendMessageResp {
    code: i64,
    msg: String,
    #[serde(default)]
    data: Option<SendMessageData>,
}

#[derive(Debug, Deserialize)]
struct SendMessageData {
    #[serde(default)]
    message_id: String,
}

struct CachedToken {
    token: String,
    expires_at: Instant,
}

/// Direct client for Feishu / Lark REST APIs and WebSocket Long Connection.
pub struct FeishuClient {
    config: FeishuConfig,
    http_client: reqwest::Client,
    status: Arc<AsyncMutex<ConnectionStatus>>,
    is_running: Arc<AtomicBool>,
    inbound_rx: Arc<AsyncMutex<Option<mpsc::Receiver<FeishuMessage>>>>,
    inbound_tx: mpsc::Sender<FeishuMessage>,
    cached_token: Arc<AsyncMutex<Option<CachedToken>>>,
    bot_open_id: Arc<AsyncMutex<Option<String>>>,
}

impl FeishuClient {
    /// Creates a new `FeishuClient` with the given configuration.
    pub fn new(config: FeishuConfig) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel::<FeishuMessage>(128);
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
            cached_token: Arc::new(AsyncMutex::new(None)),
            bot_open_id: Arc::new(AsyncMutex::new(None)),
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
    pub async fn take_message_receiver(&self) -> Option<mpsc::Receiver<FeishuMessage>> {
        self.inbound_rx.lock().await.take()
    }

    /// Retrieves a valid `tenant_access_token`, refreshing if expired.
    pub async fn get_tenant_access_token(&self) -> Result<String, FeishuError> {
        let app_id = self
            .config
            .app_id
            .as_deref()
            .ok_or(FeishuError::NotConfigured)?;
        let app_secret = self
            .config
            .app_secret
            .as_deref()
            .ok_or(FeishuError::NotConfigured)?;

        {
            let lock = self.cached_token.lock().await;
            if let Some(ref cached) = *lock {
                if Instant::now() < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        let base_url = self.config.domain.api_base_url();
        let url = format!("{}/auth/v3/tenant_access_token/internal", base_url);

        let req_body = TenantAccessTokenReq { app_id, app_secret };

        let resp = self
            .http_client
            .post(&url)
            .json(&req_body)
            .send()
            .await?;

        let data: TenantAccessTokenResp = resp.json().await?;
        if data.code != 0 {
            return Err(FeishuError::ConnectionFailed(format!(
                "Token fetch failed (code {}): {}",
                data.code, data.msg
            )));
        }

        let token = data.tenant_access_token.ok_or_else(|| {
            FeishuError::ConnectionFailed("No tenant_access_token returned by Feishu".to_string())
        })?;

        let expire_secs = data.expire.unwrap_or(7200).max(60) - 30; // 30s buffer
        let expires_at = Instant::now() + Duration::from_secs(expire_secs as u64);

        {
            let mut lock = self.cached_token.lock().await;
            *lock = Some(CachedToken {
                token: token.clone(),
                expires_at,
            });
        }

        Ok(token)
    }

    /// Validates app credentials by querying bot info.
    pub async fn test_auth(&self) -> Result<String, FeishuError> {
        let token = self.get_tenant_access_token().await?;
        let base_url = self.config.domain.api_base_url();
        let url = format!("{}/bot/v3/info", base_url);

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| FeishuError::ConnectionFailed(e.to_string()))?,
        );

        let resp = self
            .http_client
            .get(&url)
            .headers(headers)
            .send()
            .await?;

        let data: BotInfoResp = resp.json().await?;
        if data.code != 0 {
            return Err(FeishuError::ConnectionFailed(format!(
                "Bot info query failed (code {}): {}",
                data.code, data.msg
            )));
        }

        let bot = data.bot.unwrap_or(BotInfoData {
            app_name: "Operon Bot".to_string(),
            open_id: String::new(),
        });

        if !bot.open_id.is_empty() {
            let mut lock = self.bot_open_id.lock().await;
            *lock = Some(bot.open_id.clone());
        }

        Ok(format!(
            "Authenticated as '{}' ({}) on {}",
            bot.app_name, bot.open_id, self.config.domain
        ))
    }

    /// Connects to Feishu WebSocket gateway and starts streaming events.
    pub async fn connect(&self) -> Result<(), FeishuError> {
        let auth_info = self.test_auth().await?;
        info!("Feishu authentication verified: {}", auth_info);

        let app_id = self
            .config
            .app_id
            .as_deref()
            .ok_or(FeishuError::NotConfigured)?;
        let app_secret = self
            .config
            .app_secret
            .as_deref()
            .ok_or(FeishuError::NotConfigured)?;

        {
            let mut st = self.status.lock().await;
            *st = ConnectionStatus::Connecting;
        }

        let ws_url = self.config.domain.websocket_url();
        let mut request = ws_url
            .into_client_request()
            .map_err(|e| FeishuError::ConnectionFailed(e.to_string()))?;

        // Add Feishu WebSocket Long Connection handshake headers
        let headers = request.headers_mut();
        headers.insert(
            "App-Id",
            HeaderValue::from_str(app_id)
                .map_err(|e| FeishuError::ConnectionFailed(e.to_string()))?,
        );
        headers.insert(
            "App-Secret",
            HeaderValue::from_str(app_secret)
                .map_err(|e| FeishuError::ConnectionFailed(e.to_string()))?,
        );

        let (ws_stream, _) = connect_async(request)
            .await
            .map_err(|e| FeishuError::ConnectionFailed(e.to_string()))?;

        {
            let mut st = self.status.lock().await;
            *st = ConnectionStatus::Connected;
        }
        self.is_running.store(true, Ordering::SeqCst);

        let (mut write_half, mut read_half) = ws_stream.split();
        let inbound_tx = self.inbound_tx.clone();
        let status = self.status.clone();
        let is_running = self.is_running.clone();
        let bot_open_id = self.bot_open_id.lock().await.clone();

        // Spawn WebSocket listener & ping heartbeat loop
        tokio::spawn(async move {
            info!("Feishu WebSocket event listener loop started");
            let mut ping_ticker = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    msg_result = read_half.next() => {
                        match msg_result {
                            Some(Ok(WsMessage::Text(text))) => {
                                debug!("Received Feishu WebSocket frame: {}", text);
                                Self::handle_feishu_ws_frame(&text, &inbound_tx, bot_open_id.as_deref()).await;
                            }
                            Some(Ok(WsMessage::Binary(bin))) => {
                                if let Ok(text) = String::from_utf8(bin.to_vec()) {
                                    Self::handle_feishu_ws_frame(&text, &inbound_tx, bot_open_id.as_deref()).await;
                                }
                            }
                            Some(Ok(WsMessage::Ping(payload))) => {
                                let _ = write_half.send(WsMessage::Pong(payload)).await;
                            }
                            Some(Ok(WsMessage::Close(frame))) => {
                                info!("Feishu WebSocket closed by server: {:?}", frame);
                                break;
                            }
                            Some(Err(e)) => {
                                warn!("Feishu WebSocket frame error: {}", e);
                                break;
                            }
                            None => {
                                info!("Feishu WebSocket stream finished");
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = ping_ticker.tick() => {
                        if let Err(e) = write_half.send(WsMessage::Ping(vec![].into())).await {
                            warn!("Failed to send Feishu WebSocket ping: {}", e);
                            break;
                        }
                    }
                }
            }

            {
                let mut st = status.lock().await;
                *st = ConnectionStatus::Disconnected;
            }
            is_running.store(false, Ordering::SeqCst);
            info!("Feishu WebSocket listener loop exited");
        });

        Ok(())
    }

    /// Parses inbound Feishu WebSocket event frame and enqueues messages.
    async fn handle_feishu_ws_frame(
        text: &str,
        inbound_tx: &mpsc::Sender<FeishuMessage>,
        bot_open_id: Option<&str>,
    ) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
            let event_type = v
                .pointer("/header/event_type")
                .or_else(|| v.pointer("/event_type"))
                .and_then(|t| t.as_str())
                .unwrap_or_default();

            if event_type == "im.message.receive_v1" {
                if let Some(event) = v.get("event") {
                    let sender_open_id = event
                        .pointer("/sender/sender_id/open_id")
                        .or_else(|| event.pointer("/sender/sender_id/user_id"))
                        .and_then(|id| id.as_str())
                        .unwrap_or_default();

                    let sender_type = event
                        .pointer("/sender/sender_type")
                        .and_then(|s| s.as_str())
                        .unwrap_or("user");

                    let msg_obj = event.get("message");
                    let msg_id = msg_obj
                        .and_then(|m| m.get("message_id"))
                        .and_then(|id| id.as_str())
                        .unwrap_or_default();

                    let chat_id = msg_obj
                        .and_then(|m| m.get("chat_id"))
                        .and_then(|c| c.as_str())
                        .unwrap_or_default();

                    let root_id = msg_obj
                        .and_then(|m| m.get("root_id"))
                        .and_then(|r| r.as_str())
                        .map(String::from);

                    let parent_id = msg_obj
                        .and_then(|m| m.get("parent_id"))
                        .and_then(|p| p.as_str())
                        .map(String::from);

                    let msg_type = msg_obj
                        .and_then(|m| m.get("message_type"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("text");

                    let content_raw = msg_obj
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or_default();

                    // Extract text string from JSON content
                    let text_content = if msg_type == "text" {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content_raw) {
                            parsed
                                .get("text")
                                .and_then(|t| t.as_str())
                                .unwrap_or(content_raw)
                                .to_string()
                        } else {
                            content_raw.to_string()
                        }
                    } else {
                        content_raw.to_string()
                    };

                    let is_bot = sender_type == "bot"
                        || (bot_open_id.is_some() && bot_open_id == Some(sender_open_id));

                    if !is_bot && !sender_open_id.is_empty() && !text_content.is_empty() {
                        let now_secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);

                        let msg = FeishuMessage {
                            id: msg_id.to_string(),
                            chat_id: ChatId::new(chat_id),
                            author_id: UserId::new(sender_open_id),
                            text: text_content,
                            root_id,
                            parent_id,
                            timestamp: now_secs,
                            is_bot: false,
                        };

                        info!(
                            user = %msg.author_id,
                            chat = %msg.chat_id,
                            "Forwarding inbound Feishu message to router"
                        );

                        let _ = inbound_tx.send(msg).await;
                    }
                }
            }
        }
    }

    /// Sends a text message to a user or chat, optionally replying to a message.
    pub async fn send_message(
        &self,
        receive_id: &str,
        text: &str,
        reply_to_message_id: Option<&str>,
    ) -> Result<String, FeishuError> {
        let token = self.get_tenant_access_token().await?;
        let base_url = self.config.domain.api_base_url();

        let content_json = serde_json::json!({ "text": text }).to_string();

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", token))
                .map_err(|e| FeishuError::SendFailed(e.to_string()))?,
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );

        let (url, body_value) = if let Some(reply_id) = reply_to_message_id {
            let u = format!("{}/im/v1/messages/{}/reply", base_url, reply_id);
            let b = serde_json::json!({
                "msg_type": "text",
                "content": content_json,
            });
            (u, b)
        } else {
            let u = format!("{}/im/v1/messages?receive_id_type=open_id", base_url);
            let b = serde_json::json!({
                "receive_id": receive_id,
                "msg_type": "text",
                "content": content_json,
            });
            (u, b)
        };

        let resp = self
            .http_client
            .post(&url)
            .headers(headers)
            .json(&body_value)
            .send()
            .await?;

        let res_data: SendMessageResp = resp.json().await?;
        if res_data.code != 0 {
            return Err(FeishuError::SendFailed(format!(
                "Feishu message send failed (code {}): {}",
                res_data.code, res_data.msg
            )));
        }

        Ok(res_data
            .data
            .map(|d| d.message_id)
            .unwrap_or_default())
    }
}
