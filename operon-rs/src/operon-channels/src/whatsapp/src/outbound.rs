// outbound.rs — Outbound message payload formatter and markdown converter for WhatsApp.
//
// Hey friend! This file handles formatting assistant outputs for WhatsApp delivery.
// WhatsApp uses a slightly different markdown syntax than standard GitHub-Flavored Markdown:
//   - GFM `**bold**` -> WhatsApp `*bold*`
//   - GFM `*italic*` or `_italic_` -> WhatsApp `_italic_`
//   - GFM `~~strikethrough~~` -> WhatsApp `~strikethrough~`

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use serde::{Deserialize, Serialize};
use crate::types::ConnectionStatus;

/// Payload sent over the outbound queue to the WhatsApp WebSocket client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Target contact phone number / JID.
    pub recipient: String,
    /// Text payload to send.
    pub text: String,
}

impl OutboundMessage {
    /// Create a new outbound message with formatted text.
    pub fn new(recipient: &str, raw_text: &str) -> Self {
        Self {
            recipient: recipient.to_string(),
            text: format_for_whatsapp(raw_text),
        }
    }
}

/// Outbound queue buffering OutboundMessage items when disconnected and flushing FIFO when connected.
#[derive(Clone)]
pub struct OutboundQueue {
    buffer: Arc<Mutex<Vec<OutboundMessage>>>,
    tx: mpsc::Sender<OutboundMessage>,
}

impl OutboundQueue {
    /// Create a new `OutboundQueue` wrapped around an underlying mpsc channel sender.
    pub fn new(tx: mpsc::Sender<OutboundMessage>) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            tx,
        }
    }

    /// Enqueue an `OutboundMessage`. If `status` is `Connected`, sends directly;
    /// otherwise buffers the message FIFO until reconnected.
    pub async fn enqueue(
        &self,
        msg: OutboundMessage,
        status: &ConnectionStatus,
    ) -> Result<(), mpsc::error::SendError<OutboundMessage>> {
        if !matches!(status, ConnectionStatus::Disconnected) {
            // First flush any previously buffered messages in FIFO order
            let _ = self.flush().await;
            self.tx.send(msg).await
        } else {
            let mut buf = self.buffer.lock().await;
            buf.push(msg);
            Ok(())
        }
    }

    /// Flushes all buffered messages in FIFO order through the delivery channel.
    pub async fn flush(&self) -> Result<usize, mpsc::error::SendError<OutboundMessage>> {
        let mut buf = self.buffer.lock().await;
        if buf.is_empty() {
            return Ok(0);
        }

        let pending: Vec<OutboundMessage> = std::mem::take(&mut *buf);
        let count = pending.len();

        for msg in pending {
            if let Err(e) = self.tx.send(msg).await {
                let failed_msg = e.0;
                let mut buf = self.buffer.lock().await;
                buf.insert(0, failed_msg.clone());
                return Err(mpsc::error::SendError(failed_msg));
            }
        }

        Ok(count)
    }

    /// Returns the number of currently buffered messages.
    pub async fn buffered_count(&self) -> usize {
        let buf = self.buffer.lock().await;
        buf.len()
    }
}

/// Converts GFM markdown text into WhatsApp markdown format safely without panicking on multi-byte UTF-8 text.
pub fn format_for_whatsapp(input: &str) -> String {
    let mut text = input.to_string();

    // 1. Replace GFM bold `**text**` with WhatsApp `*text*`
    while let Some(start_byte) = text.find("**") {
        let search_start = start_byte + 2;
        if search_start <= text.len() {
            if let Some(end_rel_byte) = text[search_start..].find("**") {
                let end_byte = search_start + end_rel_byte;
                let inner = &text[search_start..end_byte];
                let replacement = format!("*{}*", inner);
                text.replace_range(start_byte..end_byte + 2, &replacement);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // 2. Replace GFM strikethrough `~~text~~` with WhatsApp `~text~`
    while let Some(start_byte) = text.find("~~") {
        let search_start = start_byte + 2;
        if search_start <= text.len() {
            if let Some(end_rel_byte) = text[search_start..].find("~~") {
                let end_byte = search_start + end_rel_byte;
                let inner = &text[search_start..end_byte];
                let replacement = format!("~{}~", inner);
                text.replace_range(start_byte..end_byte + 2, &replacement);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    text
}
