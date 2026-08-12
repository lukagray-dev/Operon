//! Reasoning, thinking card, and message response state controller.
//!
//! Tracks and accumulates the active streaming turns for assistant messages,
//! managing interleaved text paragraphs and thinking process cards.

use crate::main_content::markdown::ParsedMarkdownItem;
use std::collections::HashMap;
use std::time::Instant;

/// Tracks block accumulation for the active assistant message turn.
#[derive(Debug, Default, Clone)]
pub struct ResponseState {
    /// Ordered list of blocks generated within this turn.
    pub current_blocks: Vec<ParsedMarkdownItem>,

    /// Accumulates text deltas for the currently active text block.
    pub current_text_accumulator: String,

    /// Tracks whether the model was actively outputting reasoning in the last delta.
    pub in_thinking: bool,

    /// Maps tool call IDs to their work-group block and item indexes for live updates.
    pub active_tool_calls: HashMap<String, (usize, usize)>,

    /// Maps permission IDs to their index inside current_blocks for live updates.
    pub active_permissions: HashMap<String, usize>,

    /// Tracks whether the current assistant turn has an open activity summary block.
    pub work_group_open: bool,

    /// Start time used to summarize how long the current activity group ran.
    pub work_group_start: Option<Instant>,
}

impl ResponseState {
    /// Helper to get a new ResponseState instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Flushes any accumulated text deltas into a text markdown block.
    pub fn flush_text(&mut self) {
        if self.work_group_open {
            self.close_work_group();
        }

        if !self.current_text_accumulator.is_empty() {
            let text_item = ParsedMarkdownItem::new_default(
                "text".to_string(),
                self.current_text_accumulator.clone(),
            );
            self.current_blocks.push(text_item);
            self.current_text_accumulator.clear();
        }
    }

    /// Ensures a single collapsed work activity block exists for the active thinking/tool run.
    pub fn ensure_work_group_open(&mut self) -> usize {
        if !self.work_group_open {
            self.flush_text();

            let mut work_group =
                ParsedMarkdownItem::new_default("work_group".to_string(), String::new());
            work_group.work_group_active = true;
            self.current_blocks.push(work_group);
            self.work_group_open = true;
            self.work_group_start = Some(Instant::now());
        }

        self.current_blocks.len() - 1
    }

    fn close_work_group(&mut self) {
        let elapsed_secs = self
            .work_group_start
            .take()
            .map(|started| started.elapsed().as_secs().min(i32::MAX as u64) as i32)
            .unwrap_or(0);

        if let Some(group) = self
            .current_blocks
            .iter_mut()
            .rev()
            .find(|block| block.kind == "work_group" && block.work_group_active)
        {
            group.work_group_active = false;
            group.work_group_elapsed_secs = elapsed_secs;
            group.work_group_summary = build_work_summary(&group.work_group_items);

            for item in group.work_group_items.iter_mut() {
                if item.kind == "thinking" {
                    item.is_thinking_active = false;
                }
            }
        }

        self.work_group_open = false;
    }

    /// Handles a reasoning/thinking delta.
    pub fn append_thinking(&mut self, text: &str) {
        let group_idx = self.ensure_work_group_open();
        let Some(group) = self.current_blocks.get_mut(group_idx) else {
            return;
        };

        if self.in_thinking && !group.work_group_items.is_empty() {
            if let Some(last) = group.work_group_items.last_mut() {
                if last.kind == "thinking" {
                    last.thinking_text.push_str(text);
                    return;
                }
            }
        }

        let mut item = ParsedMarkdownItem::new_default("thinking".to_string(), String::new());
        item.thinking_text = text.to_string();
        item.is_thinking_active = true;
        group.work_group_items.push(item);
        self.in_thinking = true;
    }

    /// Handles a standard text delta from the assistant response.
    pub fn append_text(&mut self, text: &str) {
        if self.work_group_open {
            self.flush_text();
        }
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
        self.flush_text();
        for block in self.current_blocks.iter_mut() {
            if block.kind == "thinking" {
                block.is_thinking_active = false;
            } else if block.kind == "work_group" {
                block.work_group_active = false;
                if block.work_group_summary.is_empty() {
                    block.work_group_summary = build_work_summary(&block.work_group_items);
                }
                for item in block.work_group_items.iter_mut() {
                    if item.kind == "thinking" {
                        item.is_thinking_active = false;
                    }
                }
            }
        }
        self.current_blocks.clone()
    }
}

/// Builds the human-readable collapsed activity summary for one work group.
pub fn build_work_summary(items: &[ParsedMarkdownItem]) -> String {
    let mut listed = 0;
    let mut read = 0;
    let mut edited = 0;
    let mut searched = 0;
    let mut thought = 0;
    let mut other_tools = 0;

    for item in items {
        match item.kind.as_str() {
            "thinking" => thought += 1,
            "tool" => {
                let name = item.tool_name.as_str();
                if matches!(name, "ls" | "list_dir") {
                    listed += 1;
                } else if name == "read" {
                    read += 1;
                } else if matches!(name, "write" | "edit" | "append") {
                    edited += 1;
                } else if name.contains("search") || name.contains("web") {
                    searched += 1;
                } else {
                    other_tools += 1;
                }
            }
            _ => {}
        }
    }

    let mut parts = Vec::new();
    push_count_phrase(&mut parts, "read", read, "file", "files");
    push_count_phrase(&mut parts, "edited", edited, "file", "files");
    push_count_phrase(&mut parts, "listed", listed, "directory", "directories");
    push_times_phrase(&mut parts, "searched", searched);
    push_times_phrase(&mut parts, "thought", thought);
    push_count_phrase(&mut parts, "ran", other_tools, "tool", "tools");

    let summary = parts.join(", ");
    if summary.is_empty() {
        "Worked".to_string()
    } else {
        capitalize_first_ascii(summary)
    }
}

fn push_count_phrase(
    parts: &mut Vec<String>,
    verb: &str,
    count: i32,
    singular_noun: &str,
    plural_noun: &str,
) {
    if count == 1 {
        parts.push(format!("{verb} a {singular_noun}"));
    } else if count > 1 {
        parts.push(format!("{verb} {count} {plural_noun}"));
    }
}

fn push_times_phrase(parts: &mut Vec<String>, verb: &str, count: i32) {
    if count == 1 {
        parts.push(format!("{verb} once"));
    } else if count > 1 {
        parts.push(format!("{verb} {count} times"));
    }
}

fn capitalize_first_ascii(text: String) -> String {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return text;
    };

    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thinking_item() -> ParsedMarkdownItem {
        let mut item = ParsedMarkdownItem::new_default("thinking".to_string(), String::new());
        item.thinking_text = "Inspecting state".to_string();
        item
    }

    fn tool_item(name: &str) -> ParsedMarkdownItem {
        let mut item = ParsedMarkdownItem::new_default("tool".to_string(), String::new());
        item.tool_name = name.to_string();
        item
    }

    #[test]
    fn build_work_summary_counts_known_activity_categories() {
        let items = vec![
            tool_item("read"),
            tool_item("read"),
            tool_item("edit"),
            tool_item("ls"),
            tool_item("web_search"),
            thinking_item(),
            thinking_item(),
            thinking_item(),
        ];

        assert_eq!(
            build_work_summary(&items),
            "Read 2 files, edited a file, listed a directory, searched once, thought 3 times"
        );
    }

    #[test]
    fn build_work_summary_uses_fallback_for_unknown_tools() {
        let items = vec![tool_item("bash"), tool_item("run_command")];

        assert_eq!(build_work_summary(&items), "Ran 2 tools");
    }
}
