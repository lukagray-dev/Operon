use operon_context_normalize_messages::{ContentBlock, ConversationMessage, MessageRole};
use operon_context_normalize::tools::ToolCallId;
use serde_json::Value;

use crate::SanitizerError;

pub(crate) fn normalize_tool_calls(
    mut messages: Vec<ConversationMessage>,
) -> Result<Vec<ConversationMessage>, SanitizerError> {
    for message in &mut messages {
        if message.role != MessageRole::Assistant {
            continue;
        }

        for (position, block) in message.content.iter_mut().enumerate() {
            let ContentBlock::ToolCall(call) = block else {
                continue;
            };

            call.name = call.name.trim().to_string();

            if call.id.0.trim().is_empty() {
                call.id = ToolCallId(format!("synth_{}_{position}", call.name));
            }

            if let Value::String(raw_arguments) = &call.arguments {
                if let Ok(parsed) = serde_json::from_str::<Value>(raw_arguments) {
                    if parsed.is_object() {
                        call.arguments = parsed;
                    }
                }
            }
        }
    }

    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize::tools::{ToolCall, ToolCallId};
    use serde_json::json;

    fn assistant_with_call(id: &str, name: &str, arguments: Value) -> ConversationMessage {
        ConversationMessage::assistant(vec![ContentBlock::ToolCall(ToolCall {
            id: ToolCallId(id.to_string()),
            name: name.to_string(),
            arguments,
        })])
    }

    #[test]
    fn missing_id_is_synthesized() {
        let messages = vec![assistant_with_call("", "read_file", json!({}))];
        let output = normalize_tool_calls(messages).unwrap();

        let call = match &output[0].content[0] {
            ContentBlock::ToolCall(call) => call,
            _ => panic!("expected tool call"),
        };
        assert_eq!(call.id.0, "synth_read_file_0");
    }

    #[test]
    fn name_whitespace_is_trimmed() {
        let messages = vec![assistant_with_call("call_1", "  read_file  ", json!({}))];
        let output = normalize_tool_calls(messages).unwrap();

        let call = match &output[0].content[0] {
            ContentBlock::ToolCall(call) => call,
            _ => panic!("expected tool call"),
        };
        assert_eq!(call.name, "read_file");
    }

    #[test]
    fn json_string_arguments_are_parsed_to_object() {
        let messages = vec![assistant_with_call(
            "call_1",
            "read_file",
            Value::String("{\"path\":\"/tmp/a.txt\"}".to_string()),
        )];
        let output = normalize_tool_calls(messages).unwrap();

        let call = match &output[0].content[0] {
            ContentBlock::ToolCall(call) => call,
            _ => panic!("expected tool call"),
        };
        assert!(call.arguments.is_object());
        assert_eq!(call.arguments["path"], "/tmp/a.txt");
    }

    #[test]
    fn invalid_json_string_arguments_are_left_unchanged() {
        let messages = vec![assistant_with_call(
            "call_1",
            "read_file",
            Value::String("{invalid".to_string()),
        )];
        let output = normalize_tool_calls(messages).unwrap();

        let call = match &output[0].content[0] {
            ContentBlock::ToolCall(call) => call,
            _ => panic!("expected tool call"),
        };
        assert_eq!(call.arguments, Value::String("{invalid".to_string()));
    }
}
