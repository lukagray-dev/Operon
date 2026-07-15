//! Reasoning and thinking state controller.
//!
//! This module manages the state of the streaming assistant response.
//! It aggregates interleaved thinking and text blocks, facilitating
//! real-time rendering of multiple collapsible reasoning cards within
//! a single assistant message turn.

use crate::main_content::assistant_messages::markdown::ParsedMarkdownItem;

/// Tracks block accumulation for the active assistant message turn.
///
/// It isolates streaming logic from GUI handles, making the code testable
/// and avoiding data races.
#[derive(Debug, Default, Clone)]
pub struct ResponseState {
    /// List of completed blocks (finished thinking cards and text paragraphs).
    pub current_blocks: Vec<ParsedMarkdownItem>,
    
    /// Accumulates text deltas for the currently active text block.
    pub current_text_accumulator: String,
    
    /// Tracks whether the model was actively outputting reasoning in the last delta.
    pub in_thinking: bool,
}

impl ResponseState {
    /// Creates a new, empty response state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles a reasoning/thinking delta.
    ///
    /// If text was previously streaming, we flush it into a completed block first,
    /// then append the delta to the current thinking block (or create one).
    pub fn append_thinking(&mut self, text: &str) {
        // Hey friend! If a text block was streaming before the model switched
        // to thinking, we finalize and parse it to keep the order correct.
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(&self.current_text_accumulator);
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }

        // Hey friend! We append to the active thinking block if it's the latest block
        // in our stream, preventing layout fragmentation.
        if self.in_thinking && !self.current_blocks.is_empty() {
            if let Some(last) = self.current_blocks.last_mut() {
                if last.kind == "thinking" {
                    last.text.push_str(text);
                    return;
                }
            }
        }

        // If no active thinking block exists, we initialize a new one.
        self.current_blocks.push(ParsedMarkdownItem {
            kind: "thinking".to_string(),
            text: text.to_string(),
            lang: String::new(),
            code_lines: Vec::new(),
        });
        self.in_thinking = true;
    }

    /// Handles a standard text delta from the assistant response.
    ///
    /// It updates the text accumulator. We mark thinking as inactive.
    pub fn append_text(&mut self, text: &str) {
        self.in_thinking = false;
        self.current_text_accumulator.push_str(text);
    }

    /// Assembles the current blocks (including the streaming text) into Send-safe items.
    /// Uses lightweight streaming parsing for the active text block.
    pub fn build_parsed_items(&self) -> Vec<ParsedMarkdownItem> {
        let mut temp_blocks = self.current_blocks.clone();
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(&self.current_text_accumulator);
            temp_blocks.extend(text_items);
        }
        temp_blocks
    }

    /// Finalizes the stream, parsing any remaining text with full syntax highlighting.
    pub fn finalize(&mut self) -> Vec<ParsedMarkdownItem> {
        self.in_thinking = false;
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_sendable(&self.current_text_accumulator);
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }
        self.current_blocks.clone()
    }
}
