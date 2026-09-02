// outbound.rs — Outbound message chunking and queuing for Feishu / Lark.
//
// Hey friend! Feishu restricts text message length to 4,000 characters. This module provides:
// 1. `split_feishu_message()`: Intelligent chunking that never splits in the middle of words
//    and automatically closes/reopens markdown code blocks across split boundaries.
// 2. `OutboundQueue`: An offline buffer that holds messages during reconnections
//    and flushes them once the connection is restored.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{info, warn};

use crate::error::FeishuError;
use crate::types::ConnectionStatus;

/// Maximum safe character length for a single Feishu message payload.
pub const FEISHU_MAX_MESSAGE_LENGTH: usize = 3900;

/// Outbound message payload destined for Feishu.
#[derive(Debug, Clone)]
pub struct FeishuOutboundMessage {
    pub receive_id: String,
    pub text: String,
    pub reply_to_message_id: Option<String>,
}

impl FeishuOutboundMessage {
    /// Creates a new outbound message for a Feishu user or chat.
    pub fn new(receive_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            receive_id: receive_id.into(),
            text: text.into(),
            reply_to_message_id: None,
        }
    }

    /// Creates a new outbound message replying to a specific message ID.
    pub fn new_reply(
        receive_id: impl Into<String>,
        text: impl Into<String>,
        reply_to_message_id: Option<String>,
    ) -> Self {
        Self {
            receive_id: receive_id.into(),
            text: text.into(),
            reply_to_message_id,
        }
    }
}

/// Splits text into chunks of at most `max_len` characters, cleanly handling code blocks.
pub fn split_feishu_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        let slice = &remaining[..max_len];
        let split_idx = if let Some(pos) = slice.rfind("\n\n") {
            pos + 2
        } else if let Some(pos) = slice.rfind('\n') {
            pos + 1
        } else if let Some(pos) = slice.rfind(' ') {
            pos + 1
        } else {
            max_len
        };

        let chunk = &remaining[..split_idx];
        chunks.push(chunk.to_string());
        remaining = &remaining[split_idx..];
    }

    // Fix open code blocks across chunks
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut balanced_chunks = Vec::new();

    for chunk in chunks {
        let mut fixed_chunk = String::new();
        if in_code_block {
            if !code_lang.is_empty() {
                fixed_chunk.push_str(&format!("```{}\n", code_lang));
            } else {
                fixed_chunk.push_str("```\n");
            }
        }

        fixed_chunk.push_str(&chunk);

        // Count triple backticks in this chunk
        let mut offset = 0;
        while let Some(pos) = chunk[offset..].find("```") {
            let actual_pos = offset + pos;
            in_code_block = !in_code_block;
            if in_code_block {
                let rest = &chunk[actual_pos + 3..];
                let lang_line = rest.lines().next().unwrap_or("").trim();
                code_lang = lang_line.to_string();
            } else {
                code_lang.clear();
            }
            offset = actual_pos + 3;
        }

        if in_code_block {
            fixed_chunk.push_str("\n```");
        }

        balanced_chunks.push(fixed_chunk);
    }

    balanced_chunks
}

/// Outbound message queue with offline buffering capabilities.
pub struct OutboundQueue {
    sender: mpsc::Sender<FeishuOutboundMessage>,
    buffer: Arc<AsyncMutex<VecDeque<FeishuOutboundMessage>>>,
}

impl OutboundQueue {
    /// Creates a new `OutboundQueue`.
    pub fn new(sender: mpsc::Sender<FeishuOutboundMessage>) -> Self {
        Self {
            sender,
            buffer: Arc::new(AsyncMutex::new(VecDeque::new())),
        }
    }

    /// Enqueues an outbound message, buffering if disconnected.
    pub async fn enqueue(
        &self,
        message: FeishuOutboundMessage,
        status: &ConnectionStatus,
    ) -> Result<(), FeishuError> {
        let chunks = split_feishu_message(&message.text, FEISHU_MAX_MESSAGE_LENGTH);

        for chunk in chunks {
            let sub_msg = FeishuOutboundMessage {
                receive_id: message.receive_id.clone(),
                text: chunk,
                reply_to_message_id: message.reply_to_message_id.clone(),
            };

            match status {
                ConnectionStatus::Connected => {
                    if let Err(e) = self.sender.send(sub_msg.clone()).await {
                        warn!("Failed to send outbound message directly, buffering: {}", e);
                        let mut buf = self.buffer.lock().await;
                        buf.push_back(sub_msg);
                    }
                }
                _ => {
                    info!("Client offline, buffering outbound Feishu message");
                    let mut buf = self.buffer.lock().await;
                    buf.push_back(sub_msg);
                }
            }
        }

        Ok(())
    }

    /// Flushes all buffered messages to the outbound channel.
    pub async fn flush(&self) -> Result<usize, FeishuError> {
        let mut buf = self.buffer.lock().await;
        let count = buf.len();
        while let Some(msg) = buf.pop_front() {
            if let Err(e) = self.sender.send(msg.clone()).await {
                warn!("Error flushing buffered message: {}", e);
                buf.push_front(msg);
                break;
            }
        }
        Ok(count)
    }

    /// Returns the number of currently buffered offline messages.
    pub async fn buffered_count(&self) -> usize {
        self.buffer.lock().await.len()
    }
}

