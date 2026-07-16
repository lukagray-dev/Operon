//! Reasoning, thinking card, and message response state controller.
//!
//! Tracks and accumulates the active streaming turns for assistant messages,
//! managing interleaved text paragraphs and thinking process cards.
//!
//! Hey friend! The ResponseState here acts as the accumulator for markdown/thinking
//! blocks for the active assistant message turn. All tool call and permission
//! cards logic has been moved out to separate submodules to follow clean
//! architectural guidelines.

use crate::main_content::assistant_messages::markdown::ParsedMarkdownItem;
use std::collections::HashMap;

/// Tracks block accumulation for the active assistant message turn.
#[derive(Debug, Default, Clone)]
pub struct ResponseState {
    /// Ordered list of blocks generated within this turn.
    pub current_blocks: Vec<ParsedMarkdownItem>,
    
    /// Accumulates text deltas for the currently active text block.
    pub current_text_accumulator: String,
    
    /// Tracks whether the model was actively outputting reasoning in the last delta.
    pub in_thinking: bool,

    /// Maps tool call IDs to their index inside current_blocks for live updates.
    /// Both the streaming call_id AND the final call_id point to the same index.
    pub active_tool_calls: HashMap<String, usize>,

    /// Maps permission IDs to their index inside current_blocks for live updates.
    pub active_permissions: HashMap<String, usize>,
}

impl ResponseState {
    /// Creates a new, empty response state.
    pub fn fn_new() -> Self {
        Self::default()
    }

    /// Helper to get a new ResponseState instance.
    pub fn new() -> Self {
        Self::default()
    }

    // ─── Helper: flush pending text ────────────────────────────────────

    /// Flushes any accumulated text deltas into parsed markdown blocks.
    /// Must be called before inserting non-text blocks to preserve ordering.
    pub fn flush_text(&mut self) {
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(
                &self.current_text_accumulator,
            );
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }
    }

    // ─── Thinking ──────────────────────────────────────────────────────

    /// Handles a reasoning/thinking delta.
    pub fn append_thinking(&mut self, text: &str) {
        self.flush_text();

        // If we're already in a thinking block, just append to it
        if self.in_thinking && !self.current_blocks.is_empty() {
            if let Some(last) = self.current_blocks.last_mut() {
                if last.kind == "thinking" {
                    last.text.push_str(text);
                    return;
                }
            }
        }

        // Otherwise start a new thinking card
        self.current_blocks.push(ParsedMarkdownItem::new_default(
            "thinking".to_string(),
            text.to_string(),
            String::new(),
            Vec::new(),
        ));
        self.in_thinking = true;
    }

    // ─── Text ──────────────────────────────────────────────────────────

    /// Handles a standard text delta from the assistant response.
    pub fn append_text(&mut self, text: &str) {
        self.in_thinking = false;
        self.current_text_accumulator.push_str(text);
    }

    // ─── Snapshot builders ─────────────────────────────────────────────

    /// Assembles the current blocks (including streaming text) into Send-safe items.
    pub fn build_parsed_items(&self) -> Vec<ParsedMarkdownItem> {
        let mut temp_blocks = self.current_blocks.clone();
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(
                &self.current_text_accumulator,
            );
            temp_blocks.extend(text_items);
        }
        temp_blocks
    }

    /// Finalizes the stream, parsing any remaining text with full syntax highlighting.
    pub fn finalize(&mut self) -> Vec<ParsedMarkdownItem> {
        self.in_thinking = false;
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_sendable(
                &self.current_text_accumulator,
            );
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }
        self.current_blocks.clone()
    }
}
