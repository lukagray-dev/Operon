//! Reasoning, tool call, and permission state controller.
//!
//! Tracks and accumulates the active streaming turns for assistant messages,
//! managing interleaved text paragraphs, code blocks, thinking process cards,
//! tool execution status, and interactive permissions blocks.
//!
//! Hey friend! The key insight here is that a single tool call produces TWO
//! creation events: first ToolCallDetected (with a streaming call_id like
//! "stream-0-0") and then ToolCallStart (with the real provider call_id like
//! "toolu_01A..."). We must re-key the card on the second event to avoid
//! duplicates. The tauri reference app does the exact same re-keying trick.

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
    pub fn new() -> Self {
        Self::default()
    }

    // ─── Helper: flush pending text ────────────────────────────────────

    /// Flushes any accumulated text deltas into parsed markdown blocks.
    /// Must be called before inserting non-text blocks to preserve ordering.
    fn flush_text(&mut self) {
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

    // ─── Tool: streaming detection (early card) ────────────────────────

    /// Called on ToolCallDetected — the streaming-phase event that fires first.
    /// Creates the initial "running" card with the streaming call_id.
    pub fn append_tool_detected(&mut self, stream_call_id: &str, name: &str) {
        self.flush_text();
        self.in_thinking = false;

        let idx = self.current_blocks.len();
        let mut tool_item = ParsedMarkdownItem::new_default(
            "tool".to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        );
        tool_item.tool_name = name.to_string();
        tool_item.tool_call_id = stream_call_id.to_string();
        tool_item.tool_status = "running".to_string();
        tool_item.tool_title = get_tool_friendly_title(name, "", false);

        self.current_blocks.push(tool_item);
        self.active_tool_calls.insert(stream_call_id.to_string(), idx);
    }

    // ─── Tool: start (re-key or create) ────────────────────────────────

    /// Called on ToolCallStart — the post-parse event with the real provider
    /// call_id. If a streaming card already exists for this tool (created by
    /// ToolCallDetected), we re-key it. Otherwise we create a new card.
    pub fn append_tool_start(&mut self, call_id: &str, name: &str) {
        // Hey friend! Try to find an existing streaming card for this tool.
        // The streaming call_id has format "{turn_index}-{call_index}" while
        // the final call_id has the turn/call embedded as the last two
        // dash-separated segments (e.g. "toolu_01A-0-0").
        let stream_id = self.derive_stream_id(call_id);
        if let Some(stream_key) = stream_id {
            if let Some(&idx) = self.active_tool_calls.get(&stream_key) {
                // Re-key: remove the old streaming key and insert the final key
                self.active_tool_calls.remove(&stream_key);
                self.active_tool_calls.insert(call_id.to_string(), idx);

                // Update the card's call_id so future lookups work
                if let Some(block) = self.current_blocks.get_mut(idx) {
                    block.tool_call_id = call_id.to_string();
                }
                return; // No duplicate card created
            }
        }

        // Also check: is there already a running card with the same tool name
        // but no matching result yet? This handles edge cases where the
        // streaming id format doesn't match.
        for (&ref existing_id, &idx) in &self.active_tool_calls {
            if existing_id != call_id {
                if let Some(block) = self.current_blocks.get(idx) {
                    if block.tool_name == name && block.tool_status == "running" {
                        // Re-key this card
                        let old_id = existing_id.clone();
                        self.active_tool_calls.remove(&old_id);
                        self.active_tool_calls.insert(call_id.to_string(), idx);
                        if let Some(b) = self.current_blocks.get_mut(idx) {
                            b.tool_call_id = call_id.to_string();
                        }
                        return;
                    }
                }
            }
        }

        // No existing card found — create a fresh one
        self.flush_text();
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

    /// Derives the streaming call_id from the final provider call_id.
    /// The final call_id format is "{prefix}-{turn_index}-{call_index}",
    /// and the streaming id was stored as "{turn_index}-{call_index}".
    fn derive_stream_id(&self, call_id: &str) -> Option<String> {
        let parts: Vec<&str> = call_id.split('-').collect();
        if parts.len() >= 3 {
            let turn_index = parts[parts.len() - 2];
            let call_index = parts[parts.len() - 1];
            Some(format!("{}-{}", turn_index, call_index))
        } else {
            None
        }
    }

    // ─── Tool: args ready ──────────────────────────────────────────────

    /// Handles live parameter updates as arguments parse.
    pub fn append_tool_args_ready(&mut self, call_id: &str, name: &str, args_json: &str) {
        if let Some(&idx) = self.active_tool_calls.get(call_id) {
            if let Some(block) = self.current_blocks.get_mut(idx) {
                // Pretty-print the JSON so it reads cleanly in the card
                block.tool_args = if let Ok(val) = serde_json::from_str::<serde_json::Value>(args_json) {
                    serde_json::to_string_pretty(&val).unwrap_or_else(|_| args_json.to_string())
                } else {
                    args_json.to_string()
                };
                block.tool_title = get_tool_friendly_title(name, args_json, false);
            }
        }
    }

    // ─── Tool: body delta ──────────────────────────────────────────────

    /// Handles streaming tool body deltas (like file write content).
    pub fn append_tool_body_delta(&mut self, call_id: &str, text: &str) {
        if let Some(&idx) = self.active_tool_calls.get(call_id) {
            if let Some(block) = self.current_blocks.get_mut(idx) {
                block.tool_args.push_str(text);
            }
        }
    }

    // ─── Tool: result ──────────────────────────────────────────────────

    /// Handles the outcome of a tool execution. Updates the existing card
    /// in-place if found, otherwise creates a completed card as fallback.
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

                // Generate diff overlay for file-modifying tools
                if name == "write" || name == "append" || name == "edit" {
                    block.tool_is_diff = true;
                    let (diff_lines, added, deleted) = crate::main_content::tools::diff::parse_diff(name, &block.tool_args);
                    block.tool_diff_lines = diff_lines;
                    block.tool_added_count = added;
                    block.tool_deleted_count = deleted;
                }
            }
        } else {
            // Fallback: create a completed card if we missed both detection events
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

    // ─── Permission: approval required ─────────────────────────────────

    /// Appends a new pending permission request card.
    pub fn append_approval_required(&mut self, id: &str, tool: &str, path: &str, reason: &str, args_json: &str) {
        self.flush_text();
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

    // ─── Permission: resolved ──────────────────────────────────────────

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
        for block in self.current_blocks.iter_mut().rev() {
            if block.kind == "permission" && block.permission_status == "pending" {
                block.permission_status = "denied".to_string();
                break;
            }
        }
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

/// Generates a human-friendly tool title matching the Tauri reference layout.
pub fn get_tool_friendly_title(name: &str, args_json: &str, is_completed: bool) -> String {
    let val = crate::main_content::tools::diff::parse_tool_args_to_value(args_json);
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
                format!("Ran {}", capitalized)
            } else {
                format!("Running {}", capitalized)
            }
        }
    }
}
