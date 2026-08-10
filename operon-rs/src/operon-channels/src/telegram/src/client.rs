// client.rs — Raw reqwest Telegram Bot API long-polling client.
//
// Hey friend! This module implements `TelegramClient`, which uses raw reqwest HTTP calls
// to interact directly with Telegram's HTTPS Bot API (`getMe`, `getUpdates`, `sendMessage`).
//
// Key highlights:
//   1. No third-party framework (teloxide, frankenstein, etc.) — lightweight raw HTTP.
//   2. 30s long-polling loop (`getUpdates` with `timeout: 30`) and HTTP client timeout set to 35s.
//   3. Retries `sendMessage` once in plain text mode (omitting `parse_mode`) if `MarkdownV2` yields an HTTP 400 error.
//   4. Decoupled message receiver via `take_message_receiver()` for consumption by `TelegramService`.

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{error, info, warn};

use crate::config::TelegramConfig;
use crate::error::TelegramError;
use crate::types::{ChatId, ConnectionStatus, TelegramMessage};

// ─────────────────────────────────────────────────────────────────────────────
// Telegram Bot API JSON DTOs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
struct TelegramApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
    #[allow(dead_code)]
    error_code: Option<i32>,
}

#[derive(Debug, serde::Deserialize)]
struct UserDto {
    id: i64,
    #[allow(dead_code)]
    is_bot: bool,
    username: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ChatDto {
    id: i64,
}

#[derive(Debug, serde::Deserialize)]
struct MessageDto {
    message_id: i64,
    chat: ChatDto,
    date: i64,
    text: Option<String>,
    caption: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct UpdateDto {
    update_id: i64,
    message: Option<MessageDto>,
}

// ─────────────────────────────────────────────────────────────────────────────
// TelegramClient Definition
// ─────────────────────────────────────────────────────────────────────────────

/// Direct reqwest HTTP client for Telegram Bot API long-polling and message delivery.
pub struct TelegramClient {
    config: TelegramConfig,
    http: reqwest::Client,
    offset: Arc<AtomicI64>,
    status: Arc<RwLock<ConnectionStatus>>,
    is_running: Arc<AtomicBool>,
    message_tx: mpsc::Sender<TelegramMessage>,
    message_rx: Arc<Mutex<Option<mpsc::Receiver<TelegramMessage>>>>,
}

impl TelegramClient {
    /// Creates a new `TelegramClient` instance with the given configuration.
    pub fn new(config: TelegramConfig) -> Self {
        let (tx, rx) = mpsc::channel::<TelegramMessage>(100);
        // Build HTTP client with 35s timeout to support Telegram's 30s long-polling without client-side timeouts.
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(35))
            .build()
            .unwrap_or_default();

        Self {
            config,
            http,
            offset: Arc::new(AtomicI64::new(0)),
            status: Arc::new(RwLock::new(ConnectionStatus::Disconnected)),
            is_running: Arc::new(AtomicBool::new(false)),
            message_tx: tx,
            message_rx: Arc::new(Mutex::new(Some(rx))),
        }
    }

    /// Returns the current connection status.
    pub async fn status(&self) -> ConnectionStatus {
        self.status.read().await.clone()
    }

    /// Checks if the long-polling event loop is running.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Takes the inbound message receiver (can only be consumed once).
    pub async fn take_message_receiver(&self) -> Option<mpsc::Receiver<TelegramMessage>> {
        self.message_rx.lock().await.take()
    }

    /// Connects to Telegram by validating the bot token via `getMe` and spawning the long-poll loop.
    pub async fn connect(&self) -> Result<(), TelegramError> {
        if self.is_running() {
            info!("TelegramClient connect() called while already running.");
            return Ok(());
        }

        *self.status.write().await = ConnectionStatus::Connecting;
        let base_url = self.config.base_api_url()?;

        // Call `getMe` endpoint to verify bot token validity
        let get_me_url = format!("{}/getMe", base_url);
        info!("Validating Telegram bot token via getMe request...");

        let res = self
            .http
            .get(&get_me_url)
            .send()
            .await
            .map_err(|e| TelegramError::ConnectionFailed(format!("HTTP request failed: {e}")))?;

        let response_body: TelegramApiResponse<UserDto> = res.json().await.map_err(|e| {
            TelegramError::ConnectionFailed(format!("Failed to parse getMe JSON: {e}"))
        })?;

        if !response_body.ok || response_body.result.is_none() {
            let desc = response_body
                .description
                .unwrap_or_else(|| "Unknown Bot API error".to_string());
            let err_msg = format!("getMe validation failed: {desc}");
            *self.status.write().await = ConnectionStatus::Error(err_msg.clone());
            return Err(TelegramError::ConnectionFailed(err_msg));
        }

        let bot_user = response_body.result.unwrap();
        info!(
            bot_id = bot_user.id,
            bot_username = ?bot_user.username,
            "Telegram bot token successfully validated!"
        );

        *self.status.write().await = ConnectionStatus::Connected;
        self.is_running.store(true, Ordering::SeqCst);

        // Spawn long-polling loop task
        let http = self.http.clone();
        let base_url_clone = base_url.clone();
        let offset = self.offset.clone();
        let status = self.status.clone();
        let is_running = self.is_running.clone();
        let message_tx = self.message_tx.clone();
        let poll_timeout = self.config.poll_interval_secs.unwrap_or(30);

        tokio::spawn(async move {
            info!("Telegram long-poll loop started.");

            while is_running.load(Ordering::SeqCst) {
                let current_offset = offset.load(Ordering::SeqCst);
                let poll_payload = serde_json::json!({
                    "offset": current_offset,
                    "timeout": poll_timeout,
                    "allowed_updates": ["message"]
                });

                let get_updates_url = format!("{}/getUpdates", base_url_clone);
                let poll_res = http.post(&get_updates_url).json(&poll_payload).send().await;

                match poll_res {
                    Ok(resp) => {
                        let status_code = resp.status();
                        if !status_code.is_success() {
                            let text = resp.text().await.unwrap_or_default();
                            warn!(
                                status = %status_code,
                                body = %text,
                                "getUpdates non-200 HTTP response. Retrying in 5 seconds..."
                            );
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }

                        match resp.json::<TelegramApiResponse<Vec<UpdateDto>>>().await {
                            Ok(api_resp) => {
                                if api_resp.ok {
                                    if let Some(updates) = api_resp.result {
                                        for update in updates {
                                            // Advance offset to update_id + 1 so we don't re-fetch the same update
                                            if update.update_id >= current_offset {
                                                offset.store(update.update_id + 1, Ordering::SeqCst);
                                            }

                                            if let Some(msg) = update.message {
                                                let text_content = msg
                                                    .text
                                                    .or(msg.caption)
                                                    .unwrap_or_default();

                                                let tg_msg = TelegramMessage {
                                                    update_id: update.update_id,
                                                    message_id: msg.message_id,
                                                    sender: ChatId(msg.chat.id),
                                                    text: text_content,
                                                    timestamp: msg.date,
                                                    is_self: false,
                                                };

                                                if let Err(e) = message_tx.send(tg_msg).await {
                                                    error!("Failed to dispatch inbound Telegram message to channel: {}", e);
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    let desc = api_resp.description.unwrap_or_default();
                                    warn!("getUpdates Bot API error response: {desc}. Retrying in 5 seconds...");
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse getUpdates JSON response: {e}. Retrying in 5 seconds...");
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("getUpdates network error: {e}. Retrying in 5 seconds...");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }

            info!("Telegram long-poll loop exited.");
            *status.write().await = ConnectionStatus::Disconnected;
        });

        Ok(())
    }

    /// Sends a message to a Telegram chat.
    ///
    /// First attempts send with `parse_mode: "MarkdownV2"`.
    /// If Telegram responds with HTTP 400 (most common when Markdown escaping fails),
    /// logs the full error response body and retries ONCE with plain text (`parse_mode` omitted).
    pub async fn send_message(&self, chat_id: i64, text: &str) -> Result<i64, TelegramError> {
        if !self.is_running() {
            return Err(TelegramError::NotConnected);
        }

        let base_url = self.config.base_api_url()?;
        let send_url = format!("{}/sendMessage", base_url);

        // 1. Try with parse_mode: "MarkdownV2"
        let markdown_payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "MarkdownV2"
        });

        let res = self.http.post(&send_url).json(&markdown_payload).send().await?;
        let status_code = res.status();

        if status_code.is_success() {
            let body: TelegramApiResponse<MessageDto> = res.json().await?;
            if let Some(msg) = body.result {
                return Ok(msg.message_id);
            }
        } else if status_code == reqwest::StatusCode::BAD_REQUEST {
            // Telegram returned 400 Bad Request — typically malformed MarkdownV2
            let err_body = res.text().await.unwrap_or_default();
            warn!(
                chat_id = chat_id,
                error_response = %err_body,
                "sendMessage returned 400 Bad Request under MarkdownV2 mode. Retrying ONCE with plain text mode..."
            );

            // 2. Retry ONCE in plain text mode (omitting parse_mode)
            let plain_payload = serde_json::json!({
                "chat_id": chat_id,
                "text": text
            });

            let retry_res = self.http.post(&send_url).json(&plain_payload).send().await?;
            if retry_res.status().is_success() {
                let retry_body: TelegramApiResponse<MessageDto> = retry_res.json().await?;
                if let Some(msg) = retry_body.result {
                    info!("Successfully delivered message to chat {} using plaintext fallback!", chat_id);
                    return Ok(msg.message_id);
                }
            } else {
                let retry_err = retry_res.text().await.unwrap_or_default();
                return Err(TelegramError::SendFailed(format!(
                    "Plaintext fallback sendMessage failed: {retry_err}"
                )));
            }
        } else {
            let err_body = res.text().await.unwrap_or_default();
            return Err(TelegramError::SendFailed(format!(
                "sendMessage failed with HTTP {status_code}: {err_body}"
            )));
        }

        Err(TelegramError::SendFailed("Unexpected empty response payload from Telegram API".to_string()))
    }
}
