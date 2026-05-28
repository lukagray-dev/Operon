//! Message splitting for context compaction.
//!
//! Compaction should summarize old history while keeping recent completed turns
//! verbatim. This module only decides boundaries; it never mutates message
//! contents.

use operon_context_normalize_messages::{ConversationMessage, MessageRole};

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
    let user_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| (message.role == MessageRole::User).then_some(index))
        .collect();

    let mut preserve_start = messages.len();
    let mut complete_turns_preserved = 0usize;

    for user_position in (0..user_indices.len()).rev() {
        let user_index = user_indices[user_position];
        let turn_end = user_indices
            .get(user_position.saturating_add(1))
            .copied()
            .unwrap_or(messages.len());
        let is_last_user_turn = user_position + 1 == user_indices.len();
        let is_complete = turn_has_assistant_response(messages, user_index, turn_end);

        if !is_complete {
            // Only the final user turn is considered in-flight. Older user
            // messages without assistant responses are historical fragments and
            // can be summarized with the compactable portion.
            if is_last_user_turn {
                preserve_start = user_index;
            }
            continue;
        }

        if complete_turns_preserved >= preserved_turns {
            break;
        }

        preserve_start = user_index;
        complete_turns_preserved = complete_turns_preserved.saturating_add(1);
    }

    preserve_start
}

fn turn_has_assistant_response(
    messages: &[ConversationMessage],
    user_index: usize,
    turn_end: usize,
) -> bool {
    messages
        .get(user_index.saturating_add(1)..turn_end)
        .map(|turn_messages| {
            turn_messages
                .iter()
                .any(|message| message.role == MessageRole::Assistant)
        })
        .unwrap_or(false)
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
}
