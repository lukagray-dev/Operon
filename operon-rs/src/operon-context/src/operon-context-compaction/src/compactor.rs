//! End-to-end context compaction pipeline.
//!
//! The compactor coordinates threshold gating, history splitting, prompt
//! construction, caller-provided summarization, and rebuilt message estimation.

use operon_context_normalize_messages::{ContentBlock, ConversationMessage};
use operon_context_snapshot::SessionSnapshot;
use serde::Serialize;
use serde_json::Value;

use crate::{
    build_prompt, should_compact, split_messages, CompactionClient, CompactionConfig,
    CompactionError, CompactionResult,
};

const CHAT_MESSAGE_OVERHEAD: usize = 4;
const CODE_CHARS: &[char] = &['{', '}', '(', ')', ';', '[', ']', '<', '>', '/'];

/// Compact old conversation history into a summary plus preserved recent turns.
pub async fn compact(
    messages: Vec<ConversationMessage>,
    snapshot: &SessionSnapshot,
    client: &dyn CompactionClient,
    config: &CompactionConfig,
    used_tokens: usize,
) -> Result<CompactionResult, CompactionError> {
    if !should_compact(used_tokens, config) {
        return Err(CompactionError::ThresholdNotReached);
    }

    let split = split_messages(messages, config.preserved_turns);
    if split.compactable.is_empty() {
        return Err(CompactionError::InsufficientHistory);
    }

    let prompt = build_prompt(&split.compactable);
    let summary = client.summarize(prompt).await?;

    let mut rebuilt = Vec::with_capacity(split.preserved.len().saturating_add(2));
    rebuilt.push(ConversationMessage::system(snapshot.render()));
    rebuilt.push(ConversationMessage::assistant(vec![ContentBlock::Text(
        format!("— Previous session summary —\n\n{summary}"),
    )]));
    rebuilt.extend(split.preserved);

    let tokens_after = estimate_messages(&rebuilt)?;

    Ok(CompactionResult {
        messages: rebuilt,
        summary,
        tokens_before: used_tokens,
        tokens_after,
    })
}

fn estimate_messages(messages: &[ConversationMessage]) -> Result<usize, CompactionError> {
    messages.iter().try_fold(0usize, |total, message| {
        let message_tokens = estimate_message(message)?;
        Ok(total.saturating_add(message_tokens))
    })
}

fn estimate_message(message: &ConversationMessage) -> Result<usize, CompactionError> {
    let content_tokens = message.content.iter().try_fold(0usize, |total, block| {
        let block_tokens = estimate_block(block)?;
        Ok::<usize, CompactionError>(total.saturating_add(block_tokens))
    })?;

    Ok(content_tokens.saturating_add(CHAT_MESSAGE_OVERHEAD))
}

fn estimate_block(block: &ContentBlock) -> Result<usize, CompactionError> {
    match block {
        ContentBlock::Text(text) => Ok(estimate_text(text)),
        ContentBlock::ToolCall(call) => {
            let arguments = serde_json::to_string(&call.arguments)?;
            Ok(estimate_text(&call.name).saturating_add(estimate_text(&arguments)))
        }
        ContentBlock::ToolResult(result) => {
            let content = serializable_to_estimation_text(&result.content)?;
            Ok(estimate_text(&result.name).saturating_add(estimate_text(&content)))
        }
        ContentBlock::Reasoning(reasoning) => Ok(estimate_text(&reasoning.thinking)),
        ContentBlock::Image(image) => Ok(match &image.source {
            operon_context_normalize_messages::ImageSource::Base64 { media_type, data } => {
                estimate_text(media_type).saturating_add(estimate_text(data))
            }
            operon_context_normalize_messages::ImageSource::Url(url) => estimate_text(url),
        }),
        ContentBlock::Document(document) => {
            let title_tokens = document.title.as_deref().map(estimate_text).unwrap_or(0);
            let source_tokens = match &document.source {
                operon_context_normalize_messages::DocumentSource::Base64 { media_type, data } => {
                    estimate_text(media_type).saturating_add(estimate_text(data))
                }
                operon_context_normalize_messages::DocumentSource::Url(url) => estimate_text(url),
                operon_context_normalize_messages::DocumentSource::Text(text) => {
                    estimate_text(text)
                }
            };

            Ok(title_tokens.saturating_add(source_tokens))
        }
    }
}

fn serializable_to_estimation_text<T>(value: &T) -> Result<String, CompactionError>
where
    T: Serialize,
{
    match serde_json::to_value(value)? {
        Value::String(text) => Ok(text),
        value => Ok(serde_json::to_string(&value)?),
    }
}

fn estimate_text(text: &str) -> usize {
    if is_code_like(text) {
        (text.len() / 3).max(1)
    } else {
        (text.len() / 4).max(1)
    }
}

fn is_code_like(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let total_chars = text.chars().count();
    let code_char_count = text
        .chars()
        .filter(|character| CODE_CHARS.contains(character))
        .count();

    code_char_count.saturating_mul(100) >= total_chars.saturating_mul(15)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockCompactionClient;
    use operon_context_normalize_messages::{ContentBlock, MessageRole};
    use operon_context_snapshot::{BootstrapBlock, DirectoryTree, Role};
    use std::path::PathBuf;

    fn user(text: &str) -> ConversationMessage {
        ConversationMessage::user(vec![ContentBlock::Text(text.to_string())])
    }

    fn assistant(text: &str) -> ConversationMessage {
        ConversationMessage::assistant(vec![ContentBlock::Text(text.to_string())])
    }

    fn snapshot() -> SessionSnapshot {
        SessionSnapshot {
            bootstrap: BootstrapBlock {
                agent_name: "Codex".to_string(),
                timestamp: "2026-05-28T00:00:00Z".to_string(),
                session_id: "session-1".to_string(),
                role: Role::Owner,
            },
            agents_md: Some("Follow project instructions.".to_string()),
            tree: DirectoryTree {
                root: PathBuf::from("D:/Project Operon/Operon"),
                rendered: "src/\n  lib.rs".to_string(),
            },
            git: None,
            tool_groups: None,
        }
    }

    fn config() -> CompactionConfig {
        CompactionConfig {
            preserved_turns: 1,
            threshold_pct: 0.50,
            context_window: 1_000,
        }
    }

    #[tokio::test]
    async fn happy_path_rebuilds_system_summary_and_preserved_turns() {
        let messages = vec![
            ConversationMessage::system("old system"),
            user("old user"),
            assistant("old assistant"),
            user("recent user"),
            assistant("recent assistant"),
        ];
        let client = MockCompactionClient {
            response: "Dense summary".to_string(),
        };

        let result = compact(messages, &snapshot(), &client, &config(), 600).await;
        let result = result.unwrap_or_else(|err| panic!("compaction should succeed: {err}"));

        assert_eq!(result.messages[0].role, MessageRole::System);
        assert_eq!(result.messages[1].role, MessageRole::Assistant);
        assert_eq!(result.messages[2], user("recent user"));
        assert_eq!(result.messages[3], assistant("recent assistant"));
        assert_eq!(result.tokens_before, 600);
        assert!(result.tokens_after > 0);
    }

    #[tokio::test]
    async fn threshold_not_reached_is_returned_below_threshold() {
        let client = MockCompactionClient {
            response: "unused".to_string(),
        };
        let messages = vec![user("old user"), assistant("old assistant")];

        let result = compact(messages, &snapshot(), &client, &config(), 499).await;

        assert!(matches!(result, Err(CompactionError::ThresholdNotReached)));
    }

    #[tokio::test]
    async fn insufficient_history_is_returned_when_compactable_is_empty() {
        let client = MockCompactionClient {
            response: "unused".to_string(),
        };
        let messages = vec![
            ConversationMessage::system("old system"),
            user("only user"),
            assistant("only assistant"),
        ];

        let result = compact(messages, &snapshot(), &client, &config(), 600).await;

        assert!(matches!(result, Err(CompactionError::InsufficientHistory)));
    }

    #[tokio::test]
    async fn mock_summary_appears_verbatim_in_rebuilt_assistant_message() {
        let messages = vec![
            user("old user"),
            assistant("old assistant"),
            user("recent user"),
            assistant("recent assistant"),
        ];
        let client = MockCompactionClient {
            response: "Summary with exact path src/lib.rs".to_string(),
        };

        let result = compact(messages, &snapshot(), &client, &config(), 600).await;
        let result = result.unwrap_or_else(|err| panic!("compaction should succeed: {err}"));

        let summary_text = match &result.messages[1].content[0] {
            ContentBlock::Text(text) => text,
            other => panic!("summary message should contain text, got {other:?}"),
        };

        assert!(summary_text.contains("— Previous session summary —"));
        assert!(summary_text.contains("Summary with exact path src/lib.rs"));
    }
}
