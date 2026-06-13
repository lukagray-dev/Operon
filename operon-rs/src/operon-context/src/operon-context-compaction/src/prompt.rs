//! Summarization prompt construction.
//!
//! The prompt builder renders compactable messages into a transcript that an
//! LLM can summarize for handoff. It skips system messages because the current
//! session snapshot is injected separately by the compactor.

use operon_context_normalize_messages::{
    ContentBlock, ConversationMessage, DocumentSource, ImageSource, MessageRole,
};
use serde::Serialize;
use serde_json::Value;

const SUMMARY_INSTRUCTIONS: &str = "\
You are summarizing a previous agent session for handoff.
Produce a dense technical summary that captures:
- What the user asked for and the overall goal
- What was completed, with specific file paths, commands, and outputs
- What was decided and why (key decisions with rationale)
- What is in progress or incomplete
- Any errors encountered and how they were resolved
- Exact values that matter: file paths, variable names, IDs, counts

Do not include pleasantries, meta-commentary, or anything that does not
help an agent resume this work. Be precise, complete, and dense.
Do not truncate important details to save tokens.

CONVERSATION TO SUMMARIZE:
";

/// Build the LLM prompt used to summarize compactable history.
pub fn build_prompt(compactable: &[ConversationMessage]) -> String {
    let mut prompt = String::from(SUMMARY_INSTRUCTIONS);

    for message in compactable {
        if message.role == MessageRole::System {
            continue;
        }

        prompt.push('[');
        prompt.push_str(role_label(&message.role));
        prompt.push_str("]\n");

        for block in &message.content {
            render_block(block, &mut prompt);
        }

        prompt.push('\n');
    }

    prompt
}

fn role_label(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "USER",
        MessageRole::Assistant => "ASSISTANT",
        MessageRole::Tool => "TOOL",
        MessageRole::System => "SYSTEM",
    }
}

fn render_block(block: &ContentBlock, output: &mut String) {
    match block {
        ContentBlock::Text(text) => {
            output.push_str(text);
            output.push('\n');
        }
        ContentBlock::ToolCall(call) => {
            output.push_str("[TOOL CALL: ");
            output.push_str(&call.name);
            output.push_str("]\n");
            output.push_str(&pretty_json_or_fallback(&call.arguments));
            output.push('\n');
        }
        ContentBlock::ToolResult(result) => {
            output.push_str("[TOOL RESULT: ");
            output.push_str(&result.name);
            output.push_str("]\n");
            output.push_str(&serializable_content_or_fallback(&result.content));
            output.push('\n');
        }
        ContentBlock::Reasoning(reasoning) => {
            output.push_str("[REASONING]\n");
            output.push_str(&reasoning.thinking);
            output.push('\n');
        }
        ContentBlock::Image(image) => render_image_block(&image.source, output),
        ContentBlock::Document(document) => {
            output.push_str("[DOCUMENT");
            if let Some(title) = &document.title {
                output.push_str(": ");
                output.push_str(title);
            }
            output.push_str("]\n");
            render_document_source(&document.source, output);
        }
    }
}

fn render_image_block(source: &ImageSource, output: &mut String) {
    output.push_str("[IMAGE]\n");
    match source {
        ImageSource::Base64 { media_type, data } => {
            output.push_str("base64 media_type=");
            output.push_str(media_type);
            output.push_str(" bytes=");
            output.push_str(&data.len().to_string());
            output.push('\n');
        }
        ImageSource::Url(url) => {
            output.push_str(url);
            output.push('\n');
        }
    }
}

fn render_document_source(source: &DocumentSource, output: &mut String) {
    match source {
        DocumentSource::Base64 { media_type, data } => {
            output.push_str("base64 media_type=");
            output.push_str(media_type);
            output.push_str(" bytes=");
            output.push_str(&data.len().to_string());
            output.push('\n');
        }
        DocumentSource::Url(url) => {
            output.push_str(url);
            output.push('\n');
        }
        DocumentSource::Text(text) => {
            output.push_str(text);
            output.push('\n');
        }
    }
}

fn pretty_json_or_fallback(value: &Value) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(json) => json,
        Err(err) => format!("<failed to serialize JSON: {err}>"),
    }
}

fn serializable_content_or_fallback<T>(value: &T) -> String
where
    T: Serialize,
{
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => text,
        Ok(value) => pretty_json_or_fallback(&value),
        Err(err) => format!("<failed to serialize content: {err}>"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_reasoning::ReasoningBlock;
    use operon_context_normalize::tools::{ToolCall, ToolCallId, ToolContent, ToolResult};

    use serde_json::json;

    fn text_message(role: MessageRole, text: &str) -> ConversationMessage {
        ConversationMessage {
            role,
            content: vec![ContentBlock::Text(text.to_string())],
            stop_reason: None,
        }
    }

    #[test]
    fn output_contains_instruction_header() {
        let prompt = build_prompt(&[text_message(MessageRole::User, "implement compaction")]);

        assert!(
            prompt.starts_with("You are summarizing a previous agent session for handoff."),
            "prompt should start with the required handoff instruction"
        );
    }

    #[test]
    fn tool_calls_are_rendered_with_name_and_pretty_arguments() {
        let message = ConversationMessage::assistant(vec![ContentBlock::ToolCall(ToolCall {
            id: ToolCallId("call-1".to_string()),
            name: "read_file".to_string(),
            arguments: json!({ "path": "src/lib.rs" }),
        })]);

        let prompt = build_prompt(&[message]);

        assert!(prompt.contains("[TOOL CALL: read_file]"));
        assert!(prompt.contains("\"path\": \"src/lib.rs\""));
    }

    #[test]
    fn tool_results_are_rendered_with_content() {
        let message = ConversationMessage {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult(ToolResult {
                call_id: ToolCallId("call-1".to_string()),
                name: "read_file".to_string(),
                content: ToolContent::Text("file contents".to_string()),
                is_error: false,
                // Set to None as this is a general test result mock.
                read_paths: None,
            })],
            stop_reason: None,
        };

        let prompt = build_prompt(&[message]);

        assert!(prompt.contains("[TOOL RESULT: read_file]"));
        assert!(prompt.contains("file contents"));
    }

    #[test]
    fn system_messages_are_not_rendered_in_transcript() {
        let prompt = build_prompt(&[
            ConversationMessage::system("hidden system"),
            text_message(MessageRole::User, "visible user"),
        ]);

        assert!(!prompt.contains("hidden system"));
        assert!(prompt.contains("visible user"));
    }

    #[test]
    fn reasoning_blocks_are_rendered_with_content() {
        let message = ConversationMessage::assistant(vec![ContentBlock::Reasoning(
            ReasoningBlock::new("model thought through the dependency graph"),
        )]);

        let prompt = build_prompt(&[message]);

        assert!(prompt.contains("[REASONING]"));
        assert!(prompt.contains("model thought through the dependency graph"));
    }
}
