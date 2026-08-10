// outbound.rs — Outbound message payload formatter and MarkdownV2 converter for Telegram.
//
// Hey friend! This file handles formatting assistant outputs for Telegram delivery.
// Telegram uses MarkdownV2 syntax which requires strict character escaping and has a
// 4096-character limit per message (`TELEGRAM_MAX_MESSAGE_LENGTH`).

use crate::types::ConnectionStatus;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Telegram's maximum message length for text messages.
pub const TELEGRAM_MAX_MESSAGE_LENGTH: usize = 4096;
pub const TELEGRAM_CONTINUED_PREFIX: &str = "(continued)\n\n";
pub const TELEGRAM_CONTINUES_SUFFIX: &str = "\n\n(continues...)";
pub const TELEGRAM_FENCE_REOPEN: &str = "```\n";
pub const TELEGRAM_FENCE_CLOSE: &str = "```";

/// Payload sent over the outbound queue to the Telegram HTTP client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramOutboundMessage {
    /// Target chat ID.
    pub chat_id: i64,
    /// Text payload to send.
    pub text: String,
}

impl TelegramOutboundMessage {
    /// Create a new outbound message.
    pub fn new(chat_id: i64, text: &str) -> Self {
        Self {
            chat_id,
            text: text.to_string(),
        }
    }
}

/// Outbound queue buffering `TelegramOutboundMessage` items when disconnected and flushing FIFO when connected.
#[derive(Clone)]
pub struct OutboundQueue {
    buffer: Arc<Mutex<Vec<TelegramOutboundMessage>>>,
    tx: mpsc::Sender<TelegramOutboundMessage>,
}

impl OutboundQueue {
    /// Create a new `OutboundQueue` wrapped around an underlying mpsc channel sender.
    pub fn new(tx: mpsc::Sender<TelegramOutboundMessage>) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            tx,
        }
    }

    /// Enqueue a `TelegramOutboundMessage`.
    ///
    /// If `status` is `Connected`, flushes any previously buffered messages first.
    /// If `flush()` succeeds, sends the new message over `tx`.
    /// If `flush()` fails or if status is not `Connected`, appends the new message to the buffer to preserve strict FIFO order.
    pub async fn enqueue(
        &self,
        msg: TelegramOutboundMessage,
        status: &ConnectionStatus,
    ) -> Result<(), mpsc::error::SendError<TelegramOutboundMessage>> {
        if matches!(status, ConnectionStatus::Connected) {
            // First flush any previously buffered messages in FIFO order
            if self.flush().await.is_err() {
                // If flushing failed, socket/channel is not accepting sends. Append new message to buffer to preserve FIFO order.
                let mut buf = self.buffer.lock().await;
                buf.push(msg);
                return Ok(());
            }
            self.tx.send(msg).await
        } else {
            let mut buf = self.buffer.lock().await;
            buf.push(msg);
            Ok(())
        }
    }

    /// Flushes all buffered messages in FIFO order through the delivery channel.
    pub async fn flush(&self) -> Result<usize, mpsc::error::SendError<TelegramOutboundMessage>> {
        let pending = {
            let mut buf = self.buffer.lock().await;
            if buf.is_empty() {
                return Ok(0);
            }
            std::mem::take(&mut *buf)
        };

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

// ─────────────────────────────────────────────────────────────────────────────
// MarkdownV2 Escaping & Message Splitting
// ─────────────────────────────────────────────────────────────────────────────

/// Reserved MarkdownV2 characters requiring backslash escaping in plain text.
const RESERVED_MARKDOWN_V2_CHARS: &[char] = &[
    '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
];

/// Checks if a character is a reserved Telegram MarkdownV2 character.
fn is_reserved_markdown_char(c: char) -> bool {
    RESERVED_MARKDOWN_V2_CHARS.contains(&c)
}

/// Escapes reserved MarkdownV2 characters in plain text segments.
pub fn escape_markdown_v2_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if is_reserved_markdown_char(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Formats GFM markdown into Telegram MarkdownV2 and splits into chunks under `TELEGRAM_MAX_MESSAGE_LENGTH`.
pub fn format_for_telegram(input: &str) -> Vec<String> {
    // Step 1: Pre-process GFM syntax constructs (**bold** -> *bold*, ~~strikethrough~~ -> ~strikethrough~)
    let mut text = input.to_string();

    // Replace GFM bold `**text**` with Telegram `*text*`
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

    // Replace GFM strikethrough `~~text~~` with Telegram `~text~`
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

    // Step 2: Tokenize into code fences vs non-code segments for MarkdownV2 escaping
    let formatted = escape_segments(&text);

    // Step 3: Split into 4096-char chunks with code fence preservation
    split_message_chunks(&formatted)
}

/// Helper that tokenizes code blocks (` ``` `) and inline code (`` ` ``) from plain text,
/// escaping reserved MarkdownV2 characters in plain text while preserving code block integrity.
fn escape_segments(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    let mut cursor = 0;

    while cursor < input.len() {
        // Look for code blocks ` ``` `
        if let Some(rel_fence) = input[cursor..].find("```") {
            let fence_start = cursor + rel_fence;
            // Escape text preceding the code fence
            if fence_start > cursor {
                result.push_str(&escape_plain_text_with_formatting(&input[cursor..fence_start]));
            }

            // Find closing ` ``` `
            let code_start = fence_start + 3;
            if let Some(rel_close) = input[code_start..].find("```") {
                let fence_end = code_start + rel_close + 3;
                let code_content = &input[fence_start..fence_end];
                result.push_str(code_content);
                cursor = fence_end;
            } else {
                // Unclosed code fence — take remaining string as code block
                result.push_str(&input[fence_start..]);
                cursor = input.len();
            }
        } else {
            // Process remaining plain text
            result.push_str(&escape_plain_text_with_formatting(&input[cursor..]));
            cursor = input.len();
        }
    }

    result
}

/// Escapes plain text segments, preserving matching `*bold*`, `_italic_`, `~strikethrough~`, and `` `code` `` constructs.
fn escape_plain_text_with_formatting(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                out.push('`');
                // Collect inline code until next `
                while let Some(&ic) = chars.peek() {
                    chars.next();
                    if ic == '`' {
                        out.push('`');
                        break;
                    } else if ic == '\\' || ic == '`' {
                        out.push('\\');
                        out.push(ic);
                    } else {
                        out.push(ic);
                    }
                }
            }
            '*' | '_' | '~' => {
                // Preserve formatting delimiter but escape inner text if matching delimiter found
                out.push(c);
            }
            ch if is_reserved_markdown_char(ch) => {
                out.push('\\');
                out.push(ch);
            }
            ch => {
                out.push(ch);
            }
        }
    }

    out
}

/// Splits formatted text into chunks respecting Telegram's 4096 character limit while keeping code fences intact.
fn split_message_chunks(text: &str) -> Vec<String> {
    let char_count = text.chars().count();
    if char_count <= TELEGRAM_MAX_MESSAGE_LENGTH {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;
    let mut in_code_block = false;

    while !remaining.is_empty() {
        let remaining_chars = remaining.chars().count();
        if remaining_chars <= TELEGRAM_MAX_MESSAGE_LENGTH {
            let mut chunk = String::new();
            if in_code_block {
                chunk.push_str(TELEGRAM_FENCE_REOPEN);
            }
            chunk.push_str(remaining);
            chunks.push(chunk);
            break;
        }

        // Target budget per chunk
        let budget = TELEGRAM_MAX_MESSAGE_LENGTH - 100; // conservative headroom for prefix/suffix
        let take_chars = get_char_boundary_index(remaining, budget);
        let mut slice = &remaining[..take_chars];

        // Try to break at a newline or space if possible
        if let Some(last_nl) = slice.rfind('\n') {
            slice = &remaining[..last_nl + 1];
        } else if let Some(last_space) = slice.rfind(' ') {
            slice = &remaining[..last_space + 1];
        }

        let slice_code_blocks = count_occurrences(slice, "```");
        let ends_in_code = (in_code_block && slice_code_blocks % 2 == 0)
            || (!in_code_block && slice_code_blocks % 2 == 1);

        let mut chunk = String::new();
        if in_code_block {
            chunk.push_str(TELEGRAM_FENCE_REOPEN);
        }
        chunk.push_str(slice);

        if ends_in_code {
            if !chunk.ends_with('\n') {
                chunk.push('\n');
            }
            chunk.push_str(TELEGRAM_FENCE_CLOSE);
            in_code_block = true;
        } else {
            in_code_block = false;
        }

        chunks.push(chunk);
        remaining = &remaining[slice.len()..];
    }

    chunks
}

/// Helper that returns byte index corresponding to `count` characters safely.
fn get_char_boundary_index(s: &str, count: usize) -> usize {
    s.char_indices()
        .nth(count)
        .map_or(s.len(), |(idx, _)| idx)
}

fn count_occurrences(s: &str, pat: &str) -> usize {
    s.matches(pat).count()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_under_limit() {
        let input = "Hello world! This is a simple test message.";
        let chunks = format_for_telegram(input);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].contains("Hello world\\!"));
    }

    #[test]
    fn test_markdown_v2_escaping() {
        let input = "Special chars: . ! + - = ( ) [ ] { } # > | ~";
        let chunks = format_for_telegram(input);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].contains("\\."));
        assert!(chunks[0].contains("\\!"));
        assert!(chunks[0].contains("\\+"));
        assert!(chunks[0].contains("\\-"));
    }

    #[test]
    fn test_utf8_char_boundary_safety() {
        // Multi-byte Unicode characters (emojis, CJK, etc.) near boundaries
        let mut input = String::new();
        for _ in 0..1000 {
            input.push_str("🚀 Operon AI Assistant! 🤖 ");
        }
        let chunks = format_for_telegram(&input);
        assert!(chunks.len() >= 2);
        for chunk in chunks {
            assert!(chunk.chars().count() <= TELEGRAM_MAX_MESSAGE_LENGTH + 50);
        }
    }

    #[test]
    fn test_text_over_limit_with_code_fence() {
        let mut code_content = String::from("```rust\n");
        for i in 0..500 {
            code_content.push_str(&format!("fn test_line_{}() {{\n    println!(\"Hello!\");\n}}\n", i));
        }
        code_content.push_str("```\n");

        let chunks = format_for_telegram(&code_content);
        assert!(chunks.len() >= 2);

        // Verify synthetic code fence opening/closing on split chunks
        assert!(chunks[0].contains("```rust"));
        assert!(chunks[0].trim_end().ends_with("```"));
        assert!(chunks[1].starts_with("```\n"));
    }
}
