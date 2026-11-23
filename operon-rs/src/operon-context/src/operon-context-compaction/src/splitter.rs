//! Message splitting for context compaction.
//!
//! Compaction should summarize old history while keeping recent completed turns
//! verbatim. This module only decides boundaries; it never mutates message
//! contents.

use operon_context_normalize_messages::{ContentBlock, ConversationMessage, MessageRole};

/// Result of splitting a conversation into system, compactable, and preserved
/// portions.
#[derive(Debug, Clone)]
pub struct SplitMessages {
    /// System message from index 0. It is never summarized because the current
    /// snapshot is rendered fresh by the compactor.
    pub system: Option<ConversationMessage>,
    /// Messages eligible for summarization.
    pub compactable: Vec<ConversationMessage>,
    /// Recent complete turns, plus any final in-flight user turn, kept as-is.
    pub preserved: Vec<ConversationMessage>,
}

/// Split messages into old history to summarize and recent history to retain.
pub fn split_messages(messages: Vec<ConversationMessage>, preserved_turns: usize) -> SplitMessages {
    let (system, history) = extract_leading_system_message(messages);
    let preserve_start = find_preserved_start(&history, preserved_turns);

    let mut compactable = Vec::new();
    let mut preserved = Vec::new();

    // Consume the history once so the original `ConversationMessage` values and
    // ordering are preserved exactly in both output vectors.
    for (index, message) in history.into_iter().enumerate() {
        if index < preserve_start {
            compactable.push(message);
        } else {
            preserved.push(message);
        }
    }

    SplitMessages {
        system,
        compactable,
        preserved,
    }
}

fn extract_leading_system_message(
    messages: Vec<ConversationMessage>,
) -> (Option<ConversationMessage>, Vec<ConversationMessage>) {
    let mut iter = messages.into_iter();

    match iter.next() {
        Some(first) if first.role == MessageRole::System => (Some(first), iter.collect()),
        Some(first) => {
            let mut history = Vec::with_capacity(iter.size_hint().0.saturating_add(1));
            history.push(first);
            history.extend(iter);
            (None, history)
        }
        None => (None, Vec::new()),
    }
}

fn find_preserved_start(messages: &[ConversationMessage], preserved_turns: usize) -> usize {
    // Hey friend! Let's collect all indices where a User prompt starts.
    // Each user prompt represents the beginning of an interaction turn.
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| (message.role == MessageRole::User).then_some(index))
        .collect();

    if user_indices.is_empty() {
        return 0;
    }

    let mut preserve_start = messages.len();
    let mut complete_turns_preserved = 0usize;

    // We scan user turns from the most recent one backwards towards the beginning.
    for user_position in (0..user_indices.len()).rev() {
        let user_index = user_indices[user_position];
        let turn_end = user_indices
            .get(user_position.saturating_add(1))
            .copied()
            .unwrap_or(messages.len());
        let is_last_user_turn = user_position + 1 == user_indices.len();
        let in_flight = is_last_user_turn && is_turn_in_flight(messages, user_index, turn_end);

        if in_flight {
            // Hey friend! An active in-flight turn (e.g. the user just sent a prompt, or the
            // model is mid-turn calling tools) MUST be kept intact so execution continues smoothly.
            // It does NOT consume the user's `preserved_turns` quota, which is reserved for completed turns!
            preserve_start = user_index;
            continue;
        }

        // For completed turns, if we already kept the requested number of completed turns, we stop here.
        if complete_turns_preserved >= preserved_turns {
            break;
        }

        preserve_start = user_index;
        complete_turns_preserved = complete_turns_preserved.saturating_add(1);
    }

    preserve_start
}

/// Helper function to determine if a turn is still actively in-flight.
///
/// Hey buddy! A turn is in-flight if:
/// 1. It only has the user prompt and no response yet.
/// 2. Its latest message is not an Assistant response (e.g. it is a Tool result waiting for the assistant).
/// 3. Its latest message is an Assistant response that requested tool calls (waiting for tool execution).
///
/// A turn is only considered complete when the assistant finishes with a text/final response without pending tool calls.
fn is_turn_in_flight(
    messages: &[ConversationMessage],
    user_index: usize,
    turn_end: usize,
) -> bool {
    let turn_messages = match messages.get(user_index..turn_end) {
        Some(slice) if !slice.is_empty() => slice,
        _ => return true,
    };

    if turn_messages.len() <= 1 {
        // Hey friend! Only the user prompt exists in this turn so far. It's definitely in-flight!
        return true;
    }

    match turn_messages.last() {
        Some(last_msg) if last_msg.role != MessageRole::Assistant => {
            // Ended on a Tool result message or User message; still waiting for assistant response.
            true
        }
        Some(last_msg) => {
            // If the assistant message contains tool calls, the model is still executing tools!
            last_msg
                .content
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolCall(_)))
        }
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_messages::ContentBlock;

    fn user(text: &str) -> ConversationMessage {
        ConversationMessage::user(vec![ContentBlock::Text(text.to_string())])
    }

    fn assistant(text: &str) -> ConversationMessage {
        ConversationMessage::assistant(vec![ContentBlock::Text(text.to_string())])
    }

    #[test]
    fn system_message_is_extracted_from_index_zero() {
        let system = ConversationMessage::system("system prompt");
        let messages = vec![system.clone(), user("u1"), assistant("a1")];

        let split = split_messages(messages, 0);

        assert_eq!(split.system, Some(system));
        assert_eq!(split.compactable.len(), 2);
        assert!(split.preserved.is_empty());
    }

    #[test]
    fn last_n_complete_turns_are_preserved_verbatim() {
        let messages = vec![
            ConversationMessage::system("system"),
            user("u1"),
            assistant("a1"),
            user("u2"),
            assistant("a2"),
            assistant("a2 follow-up"),
            user("u3"),
            assistant("a3"),
        ];

        let split = split_messages(messages, 2);

        assert_eq!(split.compactable, vec![user("u1"), assistant("a1")]);
        assert_eq!(
            split.preserved,
            vec![
                user("u2"),
                assistant("a2"),
                assistant("a2 follow-up"),
                user("u3"),
                assistant("a3"),
            ]
        );
    }

    #[test]
    fn final_in_flight_user_message_is_always_preserved() {
        let messages = vec![
            ConversationMessage::system("system"),
            user("old u1"),
            assistant("old a1"),
            user("recent u2"),
            assistant("recent a2"),
            user("typing u3"),
        ];

        let split = split_messages(messages, 1);

        assert_eq!(split.compactable, vec![user("old u1"), assistant("old a1")]);
        assert_eq!(
            split.preserved,
            vec![user("recent u2"), assistant("recent a2"), user("typing u3")]
        );
    }

    #[test]
    fn compactable_is_empty_when_history_is_too_short() {
        let messages = vec![
            ConversationMessage::system("system"),
            user("only u1"),
            assistant("only a1"),
        ];

        let split = split_messages(messages, 2);

        assert!(split.compactable.is_empty());
        assert_eq!(split.preserved, vec![user("only u1"), assistant("only a1")]);
    }

    #[test]
    fn in_flight_turn_with_tool_calls_preserved_when_preserved_turns_is_zero() {
        let call_msg = ConversationMessage::assistant(vec![ContentBlock::ToolCall(
            operon_context_normalize_tools::ToolCall {
                id: operon_context_normalize_tools::ToolCallId("call_1".to_string()),
                name: "read_file".to_string(),
                arguments: serde_json::json!({"path": "src/main.rs"}),
            },
        )]);
        let result_msg = ConversationMessage {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult(
                operon_context_normalize_tools::ToolResult {
                    call_id: operon_context_normalize_tools::ToolCallId("call_1".to_string()),
                    name: "read_file".to_string(),
                    content: operon_context_normalize_tools::ToolContent::Text(
                        "fn main() {}".to_string(),
                    ),
                    is_error: false,
                },
            )],
            stop_reason: None,
        };

        let messages = vec![
            ConversationMessage::system("system"),
            user("completed u1"),
            assistant("completed a1"),
            user("active u2"),
            call_msg.clone(),
            result_msg.clone(),
        ];

        let split = split_messages(messages, 0);

        assert_eq!(
            split.compactable,
            vec![user("completed u1"), assistant("completed a1")]
        );
        assert_eq!(
            split.preserved,
            vec![user("active u2"), call_msg, result_msg]
        );
    }

    #[test]
    fn all_completed_turns_summarized_when_preserved_turns_is_zero() {
        let messages = vec![
            ConversationMessage::system("system"),
            user("u1"),
            assistant("a1"),
            user("u2"),
            assistant("a2"),
        ];

        let split = split_messages(messages, 0);

        assert_eq!(
            split.compactable,
            vec![user("u1"), assistant("a1"), user("u2"), assistant("a2")]
        );
        assert!(split.preserved.is_empty());
    }
}
