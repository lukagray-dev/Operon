use std::collections::{BTreeMap, HashMap, HashSet};

use operon_context_normalize_messages::{ContentBlock, ConversationMessage, MessageRole};

pub(crate) fn enforce_integrity(messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    let ordered = reorder_tool_results(messages);
    let merged = merge_adjacent_same_role_messages(ordered);
    deduplicate_tool_call_ids(merged)
}

fn reorder_tool_results(messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    let first_tool_call_index = first_tool_call_positions(&messages);
    let mut moved_results_by_target: BTreeMap<usize, Vec<ContentBlock>> = BTreeMap::new();
    let mut reordered_messages: Vec<ConversationMessage> = Vec::new();
    let mut output_index_by_original_index: HashMap<usize, usize> = HashMap::new();

    for (index, mut message) in messages.into_iter().enumerate() {
        let mut retained_blocks = Vec::new();

        for block in message.content {
            match block {
                ContentBlock::ToolResult(result) => {
                    let maybe_target = first_tool_call_index.get(&result.call_id.0).copied();
                    if let Some(target_index) = maybe_target {
                        if index < target_index {
                            moved_results_by_target
                                .entry(target_index)
                                .or_default()
                                .push(ContentBlock::ToolResult(result));
                            continue;
                        }
                    }
                    retained_blocks.push(ContentBlock::ToolResult(result));
                }
                other => retained_blocks.push(other),
            }
        }

        message.content = retained_blocks;
        if !message.content.is_empty() {
            output_index_by_original_index.insert(index, reordered_messages.len());
            reordered_messages.push(message);
        }
    }

    let mut insertions: Vec<(usize, ConversationMessage)> = moved_results_by_target
        .into_iter()
        .filter_map(|(target_index, moved_blocks)| {
            let output_index = output_index_by_original_index.get(&target_index).copied()?;
            Some((
                output_index + 1,
                ConversationMessage {
                    role: MessageRole::Tool,
                    content: moved_blocks,
                    stop_reason: None,
                },
            ))
        })
        .collect();

    insertions.sort_by(|left, right| right.0.cmp(&left.0));
    for (insert_index, message) in insertions {
        reordered_messages.insert(insert_index, message);
    }

    reordered_messages
}

fn first_tool_call_positions(messages: &[ConversationMessage]) -> HashMap<String, usize> {
    let mut positions = HashMap::new();

    for (index, message) in messages.iter().enumerate() {
        if message.role != MessageRole::Assistant {
            continue;
        }

        for block in &message.content {
            if let ContentBlock::ToolCall(call) = block {
                positions.entry(call.id.0.clone()).or_insert(index);
            }
        }
    }

    positions
}

fn merge_adjacent_same_role_messages(
    messages: Vec<ConversationMessage>,
) -> Vec<ConversationMessage> {
    let mut merged: Vec<ConversationMessage> = Vec::new();

    for message in messages {
        let Some(last) = merged.last_mut() else {
            merged.push(message);
            continue;
        };

        let is_same_mergeable_role = last.role == message.role
            && last.role != MessageRole::System
            && message.role != MessageRole::System;

        if is_same_mergeable_role {
            append_content_with_separator(&mut last.content, message.content);
            if message.stop_reason.is_some() {
                last.stop_reason = message.stop_reason;
            }
        } else {
            merged.push(message);
        }
    }

    merged
}

fn append_content_with_separator(base: &mut Vec<ContentBlock>, mut next: Vec<ContentBlock>) {
    if base.is_empty() {
        base.extend(next);
        return;
    }

    if next.is_empty() {
        return;
    }

    let base_ends_in_text = matches!(base.last(), Some(ContentBlock::Text(_)));
    let next_starts_with_text = matches!(next.first(), Some(ContentBlock::Text(_)));

    match (base_ends_in_text, next_starts_with_text) {
        (true, true) => {
            if let Some(ContentBlock::Text(base_tail)) = base.last_mut() {
                if let Some(ContentBlock::Text(next_head)) = next.first_mut() {
                    base_tail.push('\n');
                    base_tail.push_str(next_head);
                }
            }
            next.remove(0);
            base.extend(next);
        }
        (true, false) => {
            if let Some(ContentBlock::Text(base_tail)) = base.last_mut() {
                base_tail.push('\n');
            }
            base.extend(next);
        }
        (false, true) => {
            if let Some(ContentBlock::Text(next_head)) = next.first_mut() {
                let original = std::mem::take(next_head);
                *next_head = format!("\n{original}");
            }
            base.extend(next);
        }
        (false, false) => {
            base.push(ContentBlock::Text("\n".to_string()));
            base.extend(next);
        }
    }
}

fn deduplicate_tool_call_ids(mut messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    let mut seen_ids = HashSet::new();
    let mut duplicate_call_counts: HashMap<String, usize> = HashMap::new();

    for message in &mut messages {
        if message.role != MessageRole::Assistant {
            continue;
        }

        message.content.retain(|block| match block {
            ContentBlock::ToolCall(call) => {
                let call_id = call.id.0.clone();
                if seen_ids.insert(call_id.clone()) {
                    true
                } else {
                    *duplicate_call_counts.entry(call_id).or_insert(0) += 1;
                    false
                }
            }
            _ => true,
        });
    }

    messages.retain(|message| !message.content.is_empty());

    if duplicate_call_counts.is_empty() {
        return messages;
    }

    for message in messages.iter_mut().rev() {
        let mut index = message.content.len();
        while index > 0 {
            index -= 1;
            let should_remove = match &message.content[index] {
                ContentBlock::ToolResult(result) => {
                    if let Some(remaining_to_drop) =
                        duplicate_call_counts.get_mut(&result.call_id.0)
                    {
                        if *remaining_to_drop > 0 {
                            *remaining_to_drop -= 1;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if should_remove {
                message.content.remove(index);
            }
        }
    }

    messages.retain(|message| !message.content.is_empty());
    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_tools::{ToolCall, ToolCallId, ToolContent, ToolResult};
    use serde_json::json;

    fn user_text(text: &str) -> ConversationMessage {
        ConversationMessage::user(vec![ContentBlock::Text(text.to_string())])
    }

    fn assistant_text(text: &str) -> ConversationMessage {
        ConversationMessage::assistant(vec![ContentBlock::Text(text.to_string())])
    }

    fn assistant_call(id: &str, name: &str) -> ConversationMessage {
        ConversationMessage::assistant(vec![ContentBlock::ToolCall(ToolCall {
            id: ToolCallId(id.to_string()),
            name: name.to_string(),
            arguments: json!({}),
        })])
    }

    fn tool_result_message(id: &str, name: &str, text: &str) -> ConversationMessage {
        ConversationMessage {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId(id.to_string()),
                name: name.to_string(),
                content: ToolContent::Text(text.to_string()),
                is_error: false,
                // Set to None as this is a general test result mock.
                read_paths: None,
            })],
            stop_reason: None,
        }
    }

    fn count_tool_calls(messages: &[ConversationMessage], id: &str) -> usize {
        messages
            .iter()
            .flat_map(|message| message.content.iter())
            .filter(|block| matches!(block, ContentBlock::ToolCall(call) if call.id.0 == id))
            .count()
    }

    fn count_tool_results(messages: &[ConversationMessage], id: &str) -> usize {
        messages
            .iter()
            .flat_map(|message| message.content.iter())
            .filter(
                |block| matches!(block, ContentBlock::ToolResult(result) if result.call_id.0 == id),
            )
            .count()
    }

    #[test]
    fn adjacent_user_messages_are_merged() {
        let messages = vec![
            ConversationMessage::system("system"),
            user_text("a"),
            user_text("b"),
        ];

        let output = enforce_integrity(messages);
        assert_eq!(output.len(), 2);
        assert_eq!(output[1].role, MessageRole::User);

        let first_block = &output[1].content[0];
        assert!(matches!(first_block, ContentBlock::Text(text) if text == "a\nb"));
    }

    #[test]
    fn adjacent_assistant_messages_are_merged() {
        let messages = vec![
            ConversationMessage::system("system"),
            assistant_text("a"),
            assistant_text("b"),
        ];

        let output = enforce_integrity(messages);
        assert_eq!(output.len(), 2);
        assert_eq!(output[1].role, MessageRole::Assistant);

        let first_block = &output[1].content[0];
        assert!(matches!(first_block, ContentBlock::Text(text) if text == "a\nb"));
    }

    #[test]
    fn duplicate_tool_call_ids_drop_second_and_its_result() {
        let messages = vec![
            ConversationMessage::system("system"),
            assistant_call("dup", "read_file"),
            tool_result_message("dup", "read_file", "first"),
            assistant_call("dup", "read_file"),
            tool_result_message("dup", "read_file", "second"),
        ];

        let output = enforce_integrity(messages);
        assert_eq!(count_tool_calls(&output, "dup"), 1);
        assert_eq!(count_tool_results(&output, "dup"), 1);
    }

    #[test]
    fn reordered_tool_result_is_inserted_after_retained_target_when_prior_message_is_dropped() {
        let messages = vec![
            ConversationMessage::system("system"),
            tool_result_message("call_1", "read_file", "early result"),
            assistant_call("call_2", "read_file"),
            assistant_call("call_1", "read_file"),
            user_text("after target"),
        ];

        let output = reorder_tool_results(messages);
        let call_position = output
            .iter()
            .position(|message| count_tool_calls(std::slice::from_ref(message), "call_1") == 1)
            .unwrap();
        let result_position = output
            .iter()
            .position(|message| count_tool_results(std::slice::from_ref(message), "call_1") == 1)
            .unwrap();

        assert_eq!(result_position, call_position + 1);
    }
}
