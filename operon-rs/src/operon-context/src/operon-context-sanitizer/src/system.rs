use operon_context_normalize_messages::{ConversationMessage, MessageRole};
use operon_context_snapshot::SessionSnapshot;

pub(crate) fn inject_system(
    messages: Vec<ConversationMessage>,
    snapshot: &SessionSnapshot,
) -> Vec<ConversationMessage> {
    let mut without_system: Vec<ConversationMessage> = messages
        .into_iter()
        .filter(|message| message.role != MessageRole::System)
        .collect();

    without_system.insert(0, ConversationMessage::system(snapshot.render()));
    without_system
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_messages::ContentBlock;
    use operon_context_snapshot::{BootstrapBlock, DirectoryTree, Role, SessionSnapshot};
    use std::path::PathBuf;

    fn snapshot() -> SessionSnapshot {
        SessionSnapshot {
            bootstrap: BootstrapBlock {
                agent_name: "Operon".to_string(),
                timestamp: "2026-01-01T00:00:00Z".to_string(),
                session_id: "session".to_string(),
                role: Role::Owner,
                system_prompt: "test prompt",
            },
            agents_md: None,
            channel_instructions: None,
            tree: DirectoryTree {
                root: PathBuf::from("D:/Project Operon/Operon"),
                rendered: ".".to_string(),
            },
            git: None,
        }
    }

    #[test]
    fn stale_system_message_is_replaced() {
        let messages = vec![
            ConversationMessage::system("old system"),
            ConversationMessage::user(vec![ContentBlock::Text("user".to_string())]),
        ];

        let output = inject_system(messages, &snapshot());
        assert_eq!(output[0].role, MessageRole::System);

        let system_count = output
            .iter()
            .filter(|message| message.role == MessageRole::System)
            .count();
        assert_eq!(system_count, 1);
    }

    #[test]
    fn fresh_system_message_is_always_index_zero() {
        let messages = vec![ConversationMessage::assistant(vec![ContentBlock::Text(
            "assistant".to_string(),
        )])];

        let output = inject_system(messages, &snapshot());
        assert_eq!(output[0].role, MessageRole::System);
    }
}
