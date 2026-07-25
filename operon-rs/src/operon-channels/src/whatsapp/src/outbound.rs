// outbound.rs — Outbound message payload formatter and markdown converter for WhatsApp.
//
// Hey friend! This file handles formatting assistant outputs for WhatsApp delivery.
// WhatsApp uses a slightly different markdown syntax than standard GitHub-Flavored Markdown:
//   - GFM `**bold**` -> WhatsApp `*bold*`
//   - GFM `*italic*` or `_italic_` -> WhatsApp `_italic_`
//   - GFM `~~strikethrough~~` -> WhatsApp `~strikethrough~`

use serde::{Deserialize, Serialize};

/// Payload sent over the outbound queue to the WhatsApp WebSocket client.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Converts GFM markdown text into WhatsApp markdown format.
pub fn format_for_whatsapp(input: &str) -> String {
    let mut text = input.to_string();

    // 1. Replace GFM bold `**text**` with WhatsApp `*text*`
    while let Some(start) = text.find("**") {
        if let Some(end) = text[start + 2..].find("**") {
            let inner = &text[start + 2..start + 2 + end];
            let replacement = format!("*{}*", inner);
            text.replace_range(start..start + 2 + end + 2, &replacement);
        } else {
            break;
        }
    }

    // 2. Replace GFM strikethrough `~~text~~` with WhatsApp `~text~`
    while let Some(start) = text.find("~~") {
        if let Some(end) = text[start + 2..].find("~~") {
            let inner = &text[start + 2..start + 2 + end];
            let replacement = format!("~{}~", inner);
            text.replace_range(start..start + 2 + end + 2, &replacement);
        } else {
            break;
        }
    }

    text
}
