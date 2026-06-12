use std::collections::HashSet;

use operon_context_normalize_messages::{ContentBlock, ConversationMessage, MessageRole};

pub(crate) fn drop_orphans(messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    let without_orphan_results = drop_orphan_tool_results(messages);
    drop_orphan_tool_calls(without_orphan_results)
}

fn drop_orphan_tool_results(messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    let call_ids = collect_assistant_tool_call_ids(&messages);

    messages
        .into_iter()
        .filter_map(|mut message| {
            message.content.retain(|block| match block {
                ContentBlock::ToolResult(result) => call_ids.contains(&result.call_id.0),
                _ => true,
            });

            if message.content.is_empty() {
                None
            } else {
                Some(message)
            }
        })
        .collect()
}

fn drop_orphan_tool_calls(messages: Vec<ConversationMessage>) -> Vec<ConversationMessage> {
    let results_after_message = build_suffix_result_ids(&messages);

    messages
        .into_iter()
        .enumerate()
        .filter_map(|(index, mut message)| {
            if message.role == MessageRole::Assistant {
                let ids_after = &results_after_message[index + 1];
                message.content.retain(|block| match block {
                    ContentBlock::ToolCall(call) => ids_after.contains(&call.id.0),
                    _ => true,
                });
            }

            if message.content.is_empty() {
                None
            } else {
                Some(message)
            }
        })
        .collect()
}

fn collect_assistant_tool_call_ids(messages: &[ConversationMessage]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for message in messages {
        if message.role != MessageRole::Assistant {
            continue;
        }

        for block in &message.content {
            if let ContentBlock::ToolCall(call) = block {
                ids.insert(call.id.0.clone());
            }
        }
    }
    ids
}

fn build_suffix_result_ids(messages: &[ConversationMessage]) -> Vec<HashSet<String>> {
    let mut suffix_ids: Vec<HashSet<String>> = vec![HashSet::new(); messages.len() + 1];

    for index in (0..messages.len()).rev() {
        let mut current = suffix_ids[index + 1].clone();
        for block in &messages[index].content {
            if let ContentBlock::ToolResult(result) = block {
                current.insert(result.call_id.0.clone());
            }
        }
        suffix_ids[index] = current;
    }

    suffix_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_tools::{ToolCall, ToolCallId, ToolContent, ToolResult};
    use serde_json::json;

    fn assistant_with_call(id: &str, name: &str) -> ConversationMessage {
        ConversationMessage::assistant(vec![ContentBlock::ToolCall(ToolCall {
            id: ToolCallId(id.to_string()),
            name: name.to_string(),
            arguments: json!({}),
        })])
    }

    fn tool_message_with_result(id: &str, name: &str) -> ConversationMessage {
        ConversationMessage {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId(id.to_string()),
                name: name.to_string(),
                content: ToolContent::Text("ok".to_string()),
                is_error: false,
                // Set to None as this is a general test result mock.
                read_paths: None,
            })],
            stop_reason: None,
        }
    }

    #[test]
    fn orphan_tool_result_is_dropped() {
        let messages = vec![tool_message_with_result("missing", "read_file")];
        let output = drop_orphans(messages);
        assert!(output.is_empty());
    }

    #[test]
    fn orphan_tool_call_is_dropped() {
        let messages = vec![assistant_with_call("call_1", "read_file")];
        let output = drop_orphans(messages);
        assert!(output.is_empty());
    }

    #[test]
    fn message_with_only_orphan_blocks_is_removed() {
        let messages = vec![ConversationMessage {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId("missing".to_string()),
                name: "read_file".to_string(),
                content: ToolContent::Text("content".to_string()),
                is_error: false,
                // Set to None as this is a general test result mock.
                read_paths: None,
            })],
            stop_reason: None,
        }];

        let output = drop_orphans(messages);
        assert!(output.is_empty());
    }

    #[test]
    fn non_orphan_pairs_survive() {
        let messages = vec![
            assistant_with_call("call_1", "read_file"),
            tool_message_with_result("call_1", "read_file"),
        ];

        let output = drop_orphans(messages);
        assert_eq!(output.len(), 2);
    }
}
