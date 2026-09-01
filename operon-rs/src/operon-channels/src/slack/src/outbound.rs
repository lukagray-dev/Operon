// outbound.rs — Outbound message chunking and queuing for Slack.
//
// Hey friend! Slack restricts messages to 4,000 characters. This module provides:
// 1. `split_slack_message()`: Intelligent chunking that never splits in the middle of words
//    and automatically closes/reopens markdown code blocks across split points.
// 2. `OutboundQueue`: An offline buffer that holds messages during reconnections
//    and flushes them once the connection is restored.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{info, warn};

use crate::error::SlackError;
use crate::types::ConnectionStatus;

/// Maximum safe character length for a single Slack message payload.
pub const SLACK_MAX_MESSAGE_LENGTH: usize = 3900;

/// Outbound message payload destined for Slack.
#[derive(Debug, Clone)]
pub struct SlackOutboundMessage {
    pub channel_id: String,
    pub text: String,
    pub thread_ts: Option<String>,
}

impl SlackOutboundMessage {
    /// Creates a new outbound message for a Slack channel.
    pub fn new(channel_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            channel_id: channel_id.into(),
            text: text.into(),
            thread_ts: None,
        }
    }

    /// Creates a new outbound message targeted at a specific thread.
    pub fn new_threaded(
        channel_id: impl Into<String>,
        text: impl Into<String>,
        thread_ts: Option<String>,
    ) -> Self {
        Self {
            channel_id: channel_id.into(),
            text: text.into(),
            thread_ts,
        }
    }
}

/// Splits text into chunks of at most `max_len` characters, cleanly handling code blocks.
pub fn split_slack_message(text: &str, max_len: usize) -> Vec<String> {
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
    sender: mpsc::Sender<SlackOutboundMessage>,
    buffer: Arc<AsyncMutex<VecDeque<SlackOutboundMessage>>>,
}

impl OutboundQueue {
    /// Creates a new `OutboundQueue`.
    pub fn new(sender: mpsc::Sender<SlackOutboundMessage>) -> Self {
        Self {
            sender,
            buffer: Arc::new(AsyncMutex::new(VecDeque::new())),
        }
    }

    /// Enqueues an outbound message, buffering if disconnected.
    pub async fn enqueue(
        &self,
        message: SlackOutboundMessage,
        status: &ConnectionStatus,
    ) -> Result<(), SlackError> {
        let chunks = split_slack_message(&message.text, SLACK_MAX_MESSAGE_LENGTH);

        for chunk in chunks {
            let sub_msg = SlackOutboundMessage {
                channel_id: message.channel_id.clone(),
                text: chunk,
                thread_ts: message.thread_ts.clone(),
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
                    info!("Client offline, buffering outbound Slack message");
                    let mut buf = self.buffer.lock().await;
                    buf.push_back(sub_msg);
                }
            }
        }

        Ok(())
    }

    /// Flushes all buffered messages to the outbound channel.
    pub async fn flush(&self) -> Result<usize, SlackError> {
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
