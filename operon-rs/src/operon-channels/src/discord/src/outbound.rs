// outbound.rs — Outbound message chunking, formatting, and delivery queue for Discord.
//
// Hey friend! This module manages outbound message preparation for Discord.
//
// Discord limits standard text messages to 2,000 characters.
// `split_discord_message` breaks long assistant responses into clean, logical chunks
// (respecting code blocks and paragraph boundaries) so output never gets truncated by Discord's API.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

use crate::error::DiscordError;
use crate::types::ConnectionStatus;

/// Maximum character length for a single Discord message payload.
pub const DISCORD_MAX_MESSAGE_LENGTH: usize = 2000;

/// Represents an outbound message queued for delivery to a Discord channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordOutboundMessage {
    /// Target Discord channel snowflake ID.
    pub channel_id: String,
    /// Message body text.
    pub text: String,
}

impl DiscordOutboundMessage {
    /// Creates a new outbound message.
    pub fn new(channel_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            channel_id: channel_id.into(),
            text: text.into(),
        }
    }
}

/// Splits long text into chunks of at most `max_len` characters (default 2000 for Discord).
///
/// Intelligently preserves markdown code fences across chunk boundaries so code snippets
/// don't break rendering on Discord.
pub fn split_discord_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut in_code_block = false;
    let mut code_block_lang = String::new();

    for line in text.split_inclusive('\n') {
        // Track whether we are entering or exiting a markdown code fence
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_code_block {
                in_code_block = false;
                code_block_lang.clear();
            } else {
                in_code_block = true;
                code_block_lang = trimmed.trim_start_matches('`').trim().to_string();
            }
        }

        // If appending this line exceeds the chunk limit, flush current chunk
        if current_chunk.len() + line.len() > max_len && !current_chunk.is_empty() {
            if in_code_block {
                current_chunk.push_str("```\n");
                chunks.push(current_chunk.clone());
                current_chunk.clear();
                if !code_block_lang.is_empty() {
                    current_chunk.push_str(&format!("```{super_lang}\n", super_lang = code_block_lang));
                } else {
                    current_chunk.push_str("```\n");
                }
            } else {
                chunks.push(current_chunk.clone());
                current_chunk.clear();
            }
        }

        // If a single line by itself is longer than max_len, hard-split it
        if line.len() > max_len {
            let mut remaining = line;
            while remaining.len() > max_len {
                let (part, rest) = remaining.split_at(max_len);
                chunks.push(part.to_string());
                remaining = rest;
            }
            current_chunk.push_str(remaining);
        } else {
            current_chunk.push_str(line);
        }
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

/// Outbound delivery queue that buffers messages when disconnected and flushes when connected.
pub struct OutboundQueue {
    buffer: Arc<Mutex<VecDeque<DiscordOutboundMessage>>>,
    client_tx: mpsc::Sender<DiscordOutboundMessage>,
}

impl OutboundQueue {
    /// Creates a new `OutboundQueue` with the given client sender.
    pub fn new(client_tx: mpsc::Sender<DiscordOutboundMessage>) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            client_tx,
        }
    }

    /// Enqueues an outbound message.
    ///
    /// If `status` is `ConnectionStatus::Connected`, sends chunks directly to `client_tx`.
    /// Otherwise buffers the chunks until connection is restored.
    pub async fn enqueue(
        &self,
        msg: DiscordOutboundMessage,
        status: &ConnectionStatus,
    ) -> Result<(), DiscordError> {
        let chunks = split_discord_message(&msg.text, DISCORD_MAX_MESSAGE_LENGTH);

        if matches!(status, ConnectionStatus::Connected) {
            for chunk in chunks {
                let chunk_msg = DiscordOutboundMessage::new(&msg.channel_id, chunk);
                if let Err(e) = self.client_tx.send(chunk_msg.clone()).await {
                    warn!("Failed to dispatch Discord outbound message directly, buffering: {}", e);
                    let mut buf = self.buffer.lock().await;
                    buf.push_back(chunk_msg);
                }
            }
        } else {
            info!("Discord client is not connected, buffering outbound message chunks");
            let mut buf = self.buffer.lock().await;
            for chunk in chunks {
                buf.push_back(DiscordOutboundMessage::new(&msg.channel_id, chunk));
            }
        }

        Ok(())
    }

    /// Flushes all buffered messages to `client_tx`.
    pub async fn flush(&self) -> Result<(), DiscordError> {
        let mut buf = self.buffer.lock().await;
        while let Some(msg) = buf.pop_front() {
            if let Err(e) = self.client_tx.send(msg.clone()).await {
                error!("Failed to flush buffered Discord message: {}", e);
                buf.push_front(msg);
                break;
            }
        }
        Ok(())
    }

    /// Returns the count of currently buffered messages.
    pub async fn buffered_count(&self) -> usize {
        self.buffer.lock().await.len()
    }
}

