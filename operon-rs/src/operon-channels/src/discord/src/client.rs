// client.rs — Discord REST API and Gateway WebSocket client.
//
// Hey friend! This module implements `DiscordClient`, which handles:
//   1. REST API authentication and validation via `GET /users/@me`.
//   2. Real-time Gateway WebSocket connection (`wss://gateway.discord.gg/?v=10&encoding=json`).
//   3. Gateway Opcode 10 (Hello) handshake and periodic Opcode 1 (Heartbeat) emissions.
//   4. Opcode 2 (Identify) with message content intents.
//   5. Gateway Opcode 0 (Dispatch) event ingestion for `MESSAGE_CREATE`.
//   6. Message delivery via `POST /channels/{channel_id}/messages`.

use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as WsMessage;
use tracing::{error, info, warn};

use crate::config::DiscordConfig;
use crate::error::DiscordError;
use crate::types::{ConnectionStatus, DiscordChannelId, DiscordMessage, UserId};

// ─────────────────────────────────────────────────────────────────────────────
// Discord REST DTOs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct DiscordUserDto {
    id: String,
    username: String,
    #[serde(default)]
    bot: bool,
}

#[derive(Debug, serde::Deserialize)]
struct DiscordMessageCreateDto {
    id: String,
    channel_id: String,
    #[serde(default)]
    guild_id: Option<String>,
    content: String,
    author: DiscordUserDto,
    #[allow(dead_code)]
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GatewayHelloPayload {
    heartbeat_interval: u64,
}

#[derive(Debug, serde::Deserialize)]
struct GatewayEventPayload {
    op: u8,
    #[serde(default)]
    d: Option<serde_json::Value>,
    #[serde(default)]
    s: Option<i64>,
    #[serde(default)]
    t: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// DiscordClient Definition
// ─────────────────────────────────────────────────────────────────────────────

/// Client providing Discord REST API interactions and real-time Gateway event streaming.
pub struct DiscordClient {
    config: DiscordConfig,
    http: reqwest::Client,
    bot_id: Arc<RwLock<Option<UserId>>>,
    status: Arc<RwLock<ConnectionStatus>>,
    is_running: Arc<AtomicBool>,
    last_sequence: Arc<AtomicI64>,
    message_tx: mpsc::Sender<DiscordMessage>,
    message_rx: Arc<Mutex<Option<mpsc::Receiver<DiscordMessage>>>>,
}

impl DiscordClient {
    /// Creates a new `DiscordClient` instance with the given configuration.
    pub fn new(config: DiscordConfig) -> Self {
        let (tx, rx) = mpsc::channel::<DiscordMessage>(100);
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            config,
            http,
            bot_id: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            is_running: Arc::new(AtomicBool::new(false)),
            last_sequence: Arc::new(AtomicI64::new(0)),
            message_tx: tx,
            message_rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    /// Returns the current connection status.
    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    /// Checks if the client background loop is running.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Returns the authenticated Bot User ID once verified.
    pub async fn bot_id(&self) -> Option<UserId> {
        self.bot_id.read().await.clone()
    }

    /// Takes the inbound message receiver (can only be consumed once).
    pub async fn take_message_receiver(&self) -> Option<mpsc::Receiver<DiscordMessage>> {
        self.message_rx.lock().await.take()
    }

    /// Connects to Discord by validating the bot token via `/users/@me` and spawning the Gateway loop.
    pub async fn connect(&self) -> Result<(), DiscordError> {
        if self.is_running() {
            info!("DiscordClient connect() called while already running.");
            return Ok(());
        }

        let token = self
            .config
            .bot_token
            .as_deref()
            .ok_or(DiscordError::NotConfigured)?;

        *self.status.write().await = ConnectionStatus::Connecting;

        // 1. Verify Bot Token with REST API /users/@me
        let base_url = self.config.base_api_url();
        let me_url = format!("{}/users/@me", base_url);
        info!("Validating Discord bot token via /users/@me request...");

        let res = self
            .http
            .get(&me_url)
            .header("Authorization", format!("Bot {}", token.trim()))
            .send()
            .await
            .map_err(|e| DiscordError::ConnectionFailed(format!("HTTP request failed: {e}")))?;

        if !res.status().is_success() {
            let status_code = res.status();
            let err_body = res.text().await.unwrap_or_default();
            let err_msg = format!("Bot token validation failed with HTTP {status_code}: {err_body}");
            *self.status.write().await = ConnectionStatus::Error(err_msg.clone());
            return Err(DiscordError::ConnectionFailed(err_msg));
        }

        let bot_user: DiscordUserDto = res.json().await.map_err(|e| {
            DiscordError::ConnectionFailed(format!("Failed to parse /users/@me JSON: {e}"))
        })?;

        let current_bot_id = UserId::new(&bot_user.id);
        *self.bot_id.write().await = Some(current_bot_id.clone());

        info!(
            bot_id = %current_bot_id,
            bot_username = %bot_user.username,
            "Discord bot token successfully validated!"
        );

        *self.status.write().await = ConnectionStatus::Connected;
        self.is_running.store(true, Ordering::SeqCst);

        // 2. Spawn Discord Gateway WebSocket connection loop
        let token_str = token.trim().to_string();
        let status = self.status.clone();
        let is_running = self.is_running.clone();
        let last_seq = self.last_sequence.clone();
        let message_tx = self.message_tx.clone();
        let target_guild = self.config.guild_id.clone();
        let bot_user_id = current_bot_id;

        tokio::spawn(async move {
            info!("Starting Discord Gateway WebSocket connection task...");
            let gateway_url = "wss://gateway.discord.gg/?v=10&encoding=json";

            while is_running.load(Ordering::SeqCst) {
                info!("Connecting to Discord Gateway: {}", gateway_url);
                match connect_async(gateway_url).await {
                    Ok((ws_stream, _)) => {
                        info!("Connected to Discord Gateway WebSocket.");
                        let (mut ws_tx, mut ws_rx) = ws_stream.split();

                        // ── Await Opcode 10 Hello ──────────────────────────
                        let mut heartbeat_interval_ms = 41250u64; // Default fallback
                        if let Some(Ok(WsMessage::Text(hello_text))) = ws_rx.next().await {
                            if let Ok(event) = serde_json::from_str::<GatewayEventPayload>(&hello_text) {
                                if event.op == 10 {
                                    if let Some(d_val) = event.d {
                                        if let Ok(hello) = serde_json::from_value::<GatewayHelloPayload>(d_val) {
                                            heartbeat_interval_ms = hello.heartbeat_interval;
                                            info!("Received Gateway Hello. Heartbeat interval: {}ms", heartbeat_interval_ms);
                                        }
                                    }
                                }
                            }
                        }

                        // ── Send Opcode 2 Identify ─────────────────────────
                        // Intents: GUILDS (1) | GUILD_MESSAGES (512) | DIRECT_MESSAGES (4096) | MESSAGE_CONTENT (32768) = 37377
                        let identify_payload = serde_json::json!({
                            "op": 2,
                            "d": {
                                "token": token_str,
                                "intents": 37377,
                                "properties": {
                                    "os": "windows",
                                    "browser": "operon",
                                    "device": "operon"
                                }
                            }
                        });

                        if let Err(e) = ws_tx.send(WsMessage::Text(identify_payload.to_string().into())).await {
                            error!("Failed to send Gateway Identify payload: {}", e);
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }

                        info!("Sent Gateway Identify payload. Starting heartbeat and event consumption...");

                        // ── Spawn Periodic Heartbeat Task ──────────────────
                        let heartbeat_ws_tx = Arc::new(Mutex::new(ws_tx));
                        let heartbeat_seq = last_seq.clone();
                        let heartbeat_running = is_running.clone();
                        let hb_tx_clone = heartbeat_ws_tx.clone();

                        let hb_handle = tokio::spawn(async move {
                            let mut interval = tokio::time::interval(Duration::from_millis(heartbeat_interval_ms));
                            while heartbeat_running.load(Ordering::SeqCst) {
                                interval.tick().await;
                                let seq_val = heartbeat_seq.load(Ordering::SeqCst);
                                let hb_msg = serde_json::json!({
                                    "op": 1,
                                    "d": if seq_val > 0 { serde_json::Value::from(seq_val) } else { serde_json::Value::Null }
                                });
                                let mut lock = hb_tx_clone.lock().await;
                                if let Err(e) = lock.send(WsMessage::Text(hb_msg.to_string().into())).await {
                                    warn!("Failed to emit Gateway heartbeat: {}", e);
                                    break;
                                }
                            }
                        });

                        // ── Ingest Inbound Gateway Messages ────────────────
                        while let Some(msg_result) = ws_rx.next().await {
                            if !is_running.load(Ordering::SeqCst) {
                                break;
                            }

                            match msg_result {
                                Ok(WsMessage::Text(text)) => {
                                    if let Ok(event) = serde_json::from_str::<GatewayEventPayload>(&text) {
                                        if let Some(s) = event.s {
                                            last_seq.store(s, Ordering::SeqCst);
                                        }

                                        // Opcode 0: Dispatch
                                        if event.op == 0 {
                                            if let Some(ref event_type) = event.t {
                                                if event_type == "MESSAGE_CREATE" {
                                                    if let Some(d_val) = event.d {
                                                        if let Ok(msg_dto) = serde_json::from_value::<DiscordMessageCreateDto>(d_val) {
                                                            let author_id = UserId::new(&msg_dto.author.id);
                                                            let is_self = author_id == bot_user_id;

                                                            // Skip messages from the bot itself or other bots
                                                            if !is_self && !msg_dto.author.bot {
                                                                // If a guild_id filter is configured, enforce it
                                                                let guild_matches = match (&target_guild, &msg_dto.guild_id) {
                                                                    (Some(expected), Some(actual)) => expected == actual,
                                                                    (Some(_), None) => false, // Expected a guild message but got DM
                                                                    (None, _) => true,
                                                                };

                                                                if guild_matches {
                                                                    let timestamp_secs = std::time::SystemTime::now()
                                                                        .duration_since(std::time::UNIX_EPOCH)
                                                                        .unwrap_or_default()
                                                                        .as_secs() as i64;

                                                                    let discord_msg = DiscordMessage {
                                                                        id: msg_dto.id,
                                                                        channel_id: DiscordChannelId::new(&msg_dto.channel_id),
                                                                        author_id,
                                                                        author_username: msg_dto.author.username,
                                                                        content: msg_dto.content,
                                                                        timestamp: timestamp_secs,
                                                                        is_self: false,
                                                                        is_bot: msg_dto.author.bot,
                                                                    };

                                                                    if let Err(e) = message_tx.send(discord_msg).await {
                                                                        error!("Failed to forward inbound Discord message to channel: {}", e);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else if event.op == 7 || event.op == 9 {
                                            // Reconnect or Invalid Session
                                            warn!("Gateway requested reconnect (Opcode {}). Resetting connection...", event.op);
                                            break;
                                        }
                                    }
                                }
                                Ok(WsMessage::Close(frame)) => {
                                    warn!("Discord Gateway WebSocket closed: {:?}", frame);
                                    break;
                                }
                                Err(e) => {
                                    warn!("Discord Gateway WebSocket error: {}. Reconnecting in 5s...", e);
                                    break;
                                }
                                _ => {}
                            }
                        }

                        hb_handle.abort();
                    }
                    Err(e) => {
                        warn!("Failed to establish Discord Gateway WebSocket: {}. Retrying in 5 seconds...", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }

            info!("Discord Gateway event loop exited.");
            *status.write().await = ConnectionStatus::Disconnected;
        });

        Ok(())
    }

    /// Sends a message to a Discord channel via REST API.
    pub async fn send_message(&self, channel_id: &str, text: &str) -> Result<String, DiscordError> {
        if !self.is_running() {
            return Err(DiscordError::NotConnected);
        }

        let token = self
            .config
            .bot_token
            .as_deref()
            .ok_or(DiscordError::NotConfigured)?;

        let base_url = self.config.base_api_url();
        let send_url = format!("{}/channels/{}/messages", base_url, channel_id.trim());

        let payload = serde_json::json!({
            "content": text
        });

        let res = self
            .http
            .post(&send_url)
            .header("Authorization", format!("Bot {}", token.trim()))
            .json(&payload)
            .send()
            .await?;

        let status_code = res.status();
        if status_code.is_success() {
            let body: serde_json::Value = res.json().await?;
            if let Some(id_str) = body.get("id").and_then(|v| v.as_str()) {
                return Ok(id_str.to_string());
            }
            return Ok("ok".to_string());
        }

        let err_body = res.text().await.unwrap_or_default();
        Err(DiscordError::SendFailed(format!(
            "Discord sendMessage failed with HTTP {status_code}: {err_body}"
        )))
    }
}
