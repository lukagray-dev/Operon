//! # operon-context-sanitizer
//!
//! Cleans the conversation message array before every LLM call.
//!
//! This crate has no I/O, no async, and no side effects.
//! `sanitize()` is the single entry point.
//!
//! ## Operations (in order)
//! 1. Drop stale system messages; inject fresh snapshot as new system message
//! 2. Prepend per-turn metadata (timestamp + role) to the latest user message
//! 3. Drop orphan tool results (result with no matching tool_use)
//! 4. Drop orphan tool calls (tool_use with no matching tool_result)
//! 5. Normalize malformed assistant tool call fields
//! 6. Enforce tool ordering, role alternation, and tool-call id deduplication

mod error;
mod integrity;
mod metadata;
mod normalize;
mod orphans;
mod system;

pub use error::SanitizerError;

use operon_context_normalize_messages::ConversationMessage;
use operon_context_snapshot::{Role, SessionSnapshot};

pub fn sanitize(
    messages: Vec<ConversationMessage>,
    snapshot: &SessionSnapshot,
    role: Role,
) -> Result<Vec<ConversationMessage>, SanitizerError> {
    if messages.is_empty() {
        return Err(SanitizerError::EmptyMessages);
    }

    let messages = system::inject_system(messages, snapshot);
    let messages = metadata::inject_metadata(messages, role);
    let messages = orphans::drop_orphans(messages);
    let messages = normalize::normalize_tool_calls(messages)?;
    let messages = integrity::enforce_integrity(messages);

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_messages::{ContentBlock, ConversationMessage};
    use operon_context_snapshot::{BootstrapBlock, DirectoryTree};
    use std::path::PathBuf;

    fn snapshot() -> SessionSnapshot {
        SessionSnapshot {
            bootstrap: BootstrapBlock {
                agent_name: "Operon".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                session_id: "s1".to_string(),
                role: Role::Owner,
                system_prompt: "test prompt",
            },
            agents_md: Some("rules".to_string()),
            tree: DirectoryTree {
                root: PathBuf::from("D:/Project Operon/Operon"),
                rendered: ".".to_string(),
            },
            git: None,
            tool_groups: None,
        }
    }

    #[test]
    fn empty_input_returns_error() {
        let result = sanitize(Vec::new(), &snapshot(), Role::Owner);
        assert!(matches!(result, Err(SanitizerError::EmptyMessages)));
    }

    #[test]
    fn sanitize_injects_system_message() {
        let messages = vec![ConversationMessage::user(vec![ContentBlock::Text(
            "hello".to_string(),
        )])];

        let sanitized = sanitize(messages, &snapshot(), Role::Owner).unwrap();
        assert!(!sanitized.is_empty());
    }
}
