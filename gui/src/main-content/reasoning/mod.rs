//! Reasoning, thinking card, and message response state controller.
//!
//! Tracks and accumulates the active streaming turns for assistant messages,
//! managing interleaved text paragraphs and thinking process cards.

use crate::main_content::markdown::ParsedMarkdownItem;
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
    pub active_tool_calls: HashMap<String, usize>,

    /// Maps permission IDs to their index inside current_blocks for live updates.
    pub active_permissions: HashMap<String, usize>,
}

impl ResponseState {
    /// Helper to get a new ResponseState instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Flushes any accumulated text deltas into a text markdown block.
    pub fn flush_text(&mut self) {
        if !self.current_text_accumulator.is_empty() {
            let text_item = ParsedMarkdownItem::new_default(
                "text".to_string(),
                self.current_text_accumulator.clone(),
            );
            self.current_blocks.push(text_item);
            self.current_text_accumulator.clear();
        }
    }

    /// Handles a reasoning/thinking delta.
    pub fn append_thinking(&mut self, text: &str) {
        self.flush_text();

        if self.in_thinking && !self.current_blocks.is_empty() {
            if let Some(last) = self.current_blocks.last_mut() {
                if last.kind == "thinking" {
                    last.thinking_text.push_str(text);
                    return;
                }
            }
        }

        let mut item = ParsedMarkdownItem::new_default("thinking".to_string(), String::new());
        item.thinking_text = text.to_string();
        item.is_thinking_active = true;
        self.current_blocks.push(item);
        self.in_thinking = true;
    }

    /// Handles a standard text delta from the assistant response.
    pub fn append_text(&mut self, text: &str) {
        self.in_thinking = false;
        self.current_text_accumulator.push_str(text);
    }

    /// Assembles current blocks into a Send-safe vector of items.
    pub fn build_parsed_items(&self) -> Vec<ParsedMarkdownItem> {
        let mut temp_blocks = self.current_blocks.clone();
        if !self.current_text_accumulator.is_empty() {
            temp_blocks.push(ParsedMarkdownItem::new_default(
                "text".to_string(),
                self.current_text_accumulator.clone(),
            ));
        }
        temp_blocks
    }

    /// Finalizes the stream and returns the block list with active thinking set to false.
    pub fn finalize(&mut self) -> Vec<ParsedMarkdownItem> {
        self.in_thinking = false;
        if !self.current_text_accumulator.is_empty() {
            self.current_blocks.push(ParsedMarkdownItem::new_default(
                "text".to_string(),
                self.current_text_accumulator.clone(),
            ));
            self.current_text_accumulator.clear();
        }
        for block in self.current_blocks.iter_mut() {
            if block.kind == "thinking" {
                block.is_thinking_active = false;
            }
        }
        self.current_blocks.clone()
    }
}
