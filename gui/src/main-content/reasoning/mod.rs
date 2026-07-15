//! Reasoning, tool call, and permission state controller.
//!
//! Tracks and accumulates the active streaming turns for assistant messages,
//! managing interleaved text paragraphs, code blocks, thinking process cards,
//! tool execution status, and interactive permissions blocks.

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
    pub active_tool_calls: HashMap<String, usize>,

    /// Maps permission IDs to their index inside current_blocks for live updates.
    pub active_permissions: HashMap<String, usize>,
}

impl ResponseState {
    /// Creates a new, empty response state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles a reasoning/thinking delta.
    pub fn append_thinking(&mut self, text: &str) {
        // Hey friend! Flush any text accumulator first to preserve sequence.
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(&self.current_text_accumulator);
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }

        if self.in_thinking && !self.current_blocks.is_empty() {
            if let Some(last) = self.current_blocks.last_mut() {
                if last.kind == "thinking" {
                    last.text.push_str(text);
                    return;
                }
            }
        }

        self.current_blocks.push(ParsedMarkdownItem::new_default(
            "thinking".to_string(),
            text.to_string(),
            String::new(),
            Vec::new(),
        ));
        self.in_thinking = true;
    }

    /// Handles a standard text delta from the assistant response.
    pub fn append_text(&mut self, text: &str) {
        self.in_thinking = false;
        self.current_text_accumulator.push_str(text);
    }

    /// Handles when a tool execution is detected or started.
    pub fn append_tool_start(&mut self, call_id: &str, name: &str) {
        // Hey friend! Flush any text accumulator first to preserve sequence.
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(&self.current_text_accumulator);
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }

        self.in_thinking = false;
        
        let idx = self.current_blocks.len();
        let mut tool_item = ParsedMarkdownItem::new_default(
            "tool".to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        );
        tool_item.tool_name = name.to_string();
        tool_item.tool_call_id = call_id.to_string();
        tool_item.tool_status = "running".to_string();
        tool_item.tool_title = get_tool_friendly_title(name, "", false);

        self.current_blocks.push(tool_item);
        self.active_tool_calls.insert(call_id.to_string(), idx);
    }

    /// Handles live parameter updates as arguments parse.
    pub fn append_tool_args_ready(&mut self, call_id: &str, name: &str, args_json: &str) {
        if let Some(&idx) = self.active_tool_calls.get(call_id) {
            if let Some(block) = self.current_blocks.get_mut(idx) {
                // Try formatting arguments JSON pretty so developers can read it cleanly
                block.tool_args = if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json) {
                    serde_json::to_string_pretty(&val).unwrap_or_else(|_| args_json.to_string())
                } else {
                    args_json.to_string()
                };
                block.tool_title = get_tool_friendly_title(name, args_json, false);
            }
        }
    }

    /// Handles streaming tool body deltas (like file write streams).
    pub fn append_tool_body_delta(&mut self, call_id: &str, text: &str) {
        if let Some(&idx) = self.active_tool_calls.get(call_id) {
            if let Some(block) = self.current_blocks.get_mut(idx) {
                block.tool_args.push_str(text);
            }
        }
    }

    /// Handles the outcome of a tool execution.
    pub fn append_tool_result(&mut self, call_id: &str, name: &str, is_error: bool, content_json: &str) {
        let result_text = if let Ok(val) = serde_json::from_str::<serde_json::Value>(content_json) {
            if let Some(content) = val.get("content").and_then(|c| c.as_str()) {
                content.to_string()
            } else {
                serde_json::to_string_pretty(&val).unwrap_or_else(|_| content_json.to_string())
            }
        } else {
            content_json.to_string()
        };

        if let Some(&idx) = self.active_tool_calls.get(call_id) {
            if let Some(block) = self.current_blocks.get_mut(idx) {
                block.tool_status = if is_error { "failed".to_string() } else { "completed".to_string() };
                block.tool_result = result_text;
                
                // Re-evaluate title using final arguments string
                block.tool_title = get_tool_friendly_title(name, &block.tool_args, true);

                // Run diff generator if code tool
                if name == "write" || name == "append" || name == "edit" {
                    block.tool_is_diff = true;
                    let (diff_lines, added, deleted) = crate::main_content::tools::diff::parse_diff(name, &block.tool_args);
                    block.tool_diff_lines = diff_lines;
                    block.tool_added_count = added;
                    block.tool_deleted_count = deleted;
                }
            }
        } else {
            // Fallback: create completed card if we didn't receive ToolCallStart
            let idx = self.current_blocks.len();
            let mut tool_item = ParsedMarkdownItem::new_default(
                "tool".to_string(),
                String::new(),
                String::new(),
                Vec::new(),
            );
            tool_item.tool_name = name.to_string();
            tool_item.tool_call_id = call_id.to_string();
            tool_item.tool_status = if is_error { "failed".to_string() } else { "completed".to_string() };
            tool_item.tool_result = result_text;
            tool_item.tool_title = get_tool_friendly_title(name, "", true);

            self.current_blocks.push(tool_item);
            self.active_tool_calls.insert(call_id.to_string(), idx);
        }
    }

    /// Appends a new pending permission request card.
    pub fn append_approval_required(&mut self, id: &str, tool: &str, path: &str, reason: &str, args_json: &str) {
        // Hey friend! Flush any text accumulator first to preserve sequence.
        if !self.current_text_accumulator.is_empty() {
            let text_items = crate::main_content::assistant_messages::markdown::parse_markdown_streaming_sendable(&self.current_text_accumulator);
            self.current_blocks.extend(text_items);
            self.current_text_accumulator.clear();
        }

        self.in_thinking = false;

        let pretty_args = if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json) {
            serde_json::to_string_pretty(&val).unwrap_or_else(|_| args_json.to_string())
        } else {
            args_json.to_string()
        };

        let idx = self.current_blocks.len();
        let mut perm_item = ParsedMarkdownItem::new_default(
            "permission".to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        );
        perm_item.permission_id = id.to_string();
        perm_item.permission_tool = tool.to_string();
        perm_item.permission_path = path.to_string();
        perm_item.permission_reason = reason.to_string();
        perm_item.permission_args = pretty_args;
        perm_item.permission_status = "pending".to_string();

        self.current_blocks.push(perm_item);
        self.active_permissions.insert(id.to_string(), idx);
    }

    /// Resolves an active permission request.
    pub fn append_approval_resolved(&mut self, id: &str, approved: bool) {
        if let Some(&idx) = self.active_permissions.get(id) {
            if let Some(block) = self.current_blocks.get_mut(idx) {
                block.permission_status = if approved { "approved".to_string() } else { "denied".to_string() };
            }
        }
    }

    /// Marks the latest pending permission request as denied when flatly rejected by policy.
    pub fn append_permission_denied_event(&mut self, _tool: &str, _path: &str, _reason: &str) {
        // Hey friend! Since policy denials are synchronous and flat, we find the latest
        // pending permission card and mark it as denied.
        for block in self.current_blocks.iter_mut().rev() {
            if block.kind == "permission" && block.permission_status == "pending" {
                block.permission_status = "denied".to_string();
                break;
            }
        }
    }

    /// Assembles the current blocks (including streaming text) into Send-safe items.
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

/// Generates a human-friendly tool title matching the Svelte layout guidelines.
pub fn get_tool_friendly_title(name: &str, args_json: &str, is_completed: bool) -> String {
    let val: serde_json::Value = serde_json::from_str(args_json).unwrap_or_default();
    let path = val.get("path")
        .or_else(|| val.get("paths"))
        .or_else(|| val.get("dir"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
        
    let display_name = if !path.is_empty() {
        let mut path_entries = Vec::new();
        if path.contains('\n') {
            path_entries = path.split('\n').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
        } else {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                path_entries.push(trimmed);
            }
        }
        
        let file_names: Vec<String> = path_entries.into_iter().map(|p| {
            // Strip optional line ranges like :40-90 or :50-
            let clean_p = if let Some(idx) = p.rfind(':') {
                if p[idx+1..].chars().all(|c| c.is_ascii_digit() || c == '-') {
                    &p[..idx]
                } else {
                    p
                }
            } else {
                p
            };
            let parts: Vec<&str> = clean_p.split(|c| c == '/' || c == '\\').collect();
            parts.last().copied().unwrap_or(clean_p).to_string()
        }).collect();
        
        file_names.join(", ")
    } else {
        String::new()
    };

    match name {
        "write" => {
            if is_completed {
                format!("Wrote {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Writing {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "append" => {
            if is_completed {
                format!("Appended {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Appending {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "edit" => {
            if is_completed {
                format!("Edited {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Editing {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "read" => {
            if is_completed {
                format!("Read {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Reading {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "delete" => {
            if is_completed {
                format!("Deleted {}", if display_name.is_empty() { "file" } else { &display_name })
            } else {
                format!("Deleting {}", if display_name.is_empty() { "file" } else { &display_name })
            }
        }
        "ls" => {
            if is_completed {
                format!("Listed {}", if display_name.is_empty() { "directory" } else { &display_name })
            } else {
                format!("Listing {}", if display_name.is_empty() { "directory" } else { &display_name })
            }
        }
        "grep" => {
            if is_completed {
                format!("Searched {}", if display_name.is_empty() { "directory" } else { &display_name })
            } else {
                format!("Searching {}", if display_name.is_empty() { "directory" } else { &display_name })
            }
        }
        "bash" => {
            if is_completed { "Executed command".to_string() } else { "Executing command".to_string() }
        }
        "ask" => {
            if is_completed { "Asked question".to_string() } else { "Asking question".to_string() }
        }
        "web_search" => {
            if is_completed { "Searched web".to_string() } else { "Searching web".to_string() }
        }
        "web_fetch" => {
            if is_completed { "Fetched web page".to_string() } else { "Fetching web page".to_string() }
        }
        "todo_create" => {
            if is_completed { "Created TODO".to_string() } else { "Creating TODO".to_string() }
        }
        "todo_update" => {
            if is_completed { "Updated TODO".to_string() } else { "Updating TODO".to_string() }
        }
        "todo_list" => {
            if is_completed { "Listed TODOs".to_string() } else { "Listing TODOs".to_string() }
        }
        _ => {
            let mut chars = name.chars();
            let capitalized = match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            };
            if is_completed {
                format!("Finished {}", capitalized)
            } else {
                format!("Running {}", capitalized)
            }
        }
    }
}
