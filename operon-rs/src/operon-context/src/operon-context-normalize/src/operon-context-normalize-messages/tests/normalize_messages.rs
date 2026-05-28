//! Integration tests for `operon-context-normalize-messages`.
//!
//! These tests validate provider-specific wire parsing and rendering for all
//! supported providers.

use operon_context_normalize_messages::{
    denormalize_messages, normalize_message, ContentBlock, ConversationMessage, MessageNormalizeError,
    MessageRole, Provider, StopReason,
};
use operon_context_normalize_reasoning::ReasoningBlock;
use operon_context_normalize_tools::{ToolCall, ToolCallId, ToolContent, ToolResult};
use serde_json::json;

fn sample_tool_call() -> ToolCall {
    ToolCall {
        id: ToolCallId("call_123".to_string()),
        name: "read_file".to_string(),
        arguments: json!({"path": "/tmp/a.txt"}),
    }
}

fn sample_tool_result() -> ToolResult {
    ToolResult {
        call_id: ToolCallId("call_123".to_string()),
        name: "read_file".to_string(),
        content: ToolContent::Text("hello".to_string()),
        is_error: false,
    }
}

#[test]
fn anthropic_plain_text_user() {
    let raw = json!({"role":"user","content":"hello"});
    let msg = normalize_message(raw, &Provider::Anthropic).unwrap();
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content, vec![ContentBlock::Text("hello".to_string())]);
}

#[test]
fn anthropic_assistant_stop_reason() {
    let raw = json!({
        "role": "assistant",
        "content": [{"type":"text","text":"done"}],
        "stop_reason": "end_turn"
    });
    let msg = normalize_message(raw, &Provider::Anthropic).unwrap();
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.stop_reason, Some(StopReason::EndTurn));
}

#[test]
fn anthropic_tool_call_and_reasoning_and_tool_result() {
    let assistant = json!({
        "role":"assistant",
        "content":[
            {"type":"thinking","thinking":"let me think"},
            {"type":"tool_use","id":"toolu_1","name":"read_file","input":{"path":"/tmp/a.txt"}}
        ]
    });
    let msg = normalize_message(assistant, &Provider::Anthropic).unwrap();
    assert!(matches!(msg.content[0], ContentBlock::Reasoning(_)));
    assert!(matches!(msg.content[1], ContentBlock::ToolCall(_)));

    let user = json!({
        "role":"user",
        "content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"ok","is_error":false}]
    });
    let tool_msg = normalize_message(user, &Provider::Anthropic).unwrap();
    assert!(matches!(tool_msg.content[0], ContentBlock::ToolResult(_)));
}

#[test]
fn anthropic_image_blocks() {
    let raw = json!({
        "role":"user",
        "content":[
            {"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"abc"}},
            {"type":"image","source":{"type":"url","url":"https://example.com/a.jpg"}}
        ]
    });
    let msg = normalize_message(raw, &Provider::Anthropic).unwrap();
    assert!(matches!(msg.content[0], ContentBlock::Image(_)));
    assert!(matches!(msg.content[1], ContentBlock::Image(_)));
}

#[test]
fn anthropic_system_field() {
    let raw = json!({"system":"You are helpful"});
    let msg = normalize_message(raw, &Provider::Anthropic).unwrap();
    assert_eq!(msg.role, MessageRole::System);
    assert_eq!(msg.content, vec![ContentBlock::Text("You are helpful".to_string())]);
}

#[test]
fn anthropic_denormalize_shape() {
    let msgs = vec![
        ConversationMessage::system("You are helpful"),
        ConversationMessage::user(vec![ContentBlock::Text("hello".to_string())]),
        ConversationMessage::assistant(vec![ContentBlock::ToolCall(sample_tool_call())]),
    ];
    let wire = denormalize_messages(&msgs, &Provider::Anthropic).unwrap();
    assert!(wire.get("messages").unwrap().is_array());
    assert_eq!(wire["system"], "You are helpful");
}

#[test]
fn anthropic_missing_required_field() {
    let raw = json!({
        "role":"assistant",
        "content":[{"type":"tool_use","name":"read_file","input":{}}]
    });
    let err = normalize_message(raw, &Provider::Anthropic).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::MissingField { field: "id", provider: "Anthropic" }));
}

#[test]
fn anthropic_unknown_role() {
    let raw = json!({"role":"developer","content":"x"});
    let err = normalize_message(raw, &Provider::Anthropic).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnknownRole { provider: "Anthropic", .. }));
}

#[test]
fn openai_plain_text_user() {
    let raw = json!({"role":"user","content":"hello"});
    let msg = normalize_message(raw, &Provider::OpenAI).unwrap();
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content, vec![ContentBlock::Text("hello".to_string())]);
}

#[test]
fn openai_assistant_stop_reason() {
    let raw = json!({
        "choices": [{
            "message": {"role":"assistant","content":"done"},
            "finish_reason":"stop"
        }]
    });
    let msg = normalize_message(raw, &Provider::OpenAI).unwrap();
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.stop_reason, Some(StopReason::EndTurn));
}

#[test]
fn openai_tool_call_and_null_content() {
    let raw = json!({
        "role":"assistant",
        "content": null,
        "tool_calls":[
            {"id":"call_123","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"/tmp/a.txt\"}"}}
        ]
    });
    let msg = normalize_message(raw, &Provider::OpenAI).unwrap();
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_))));
}

#[test]
fn openai_images_data_uri_and_url() {
    let raw = json!({
        "role":"user",
        "content":[
            {"type":"text","text":"what is this"},
            {"type":"image_url","image_url":{"url":"data:image/jpeg;base64,abc"}},
            {"type":"image_url","image_url":{"url":"https://example.com/a.jpg"}}
        ]
    });
    let msg = normalize_message(raw, &Provider::OpenAI).unwrap();
    assert!(matches!(msg.content[1], ContentBlock::Image(_)));
    assert!(matches!(msg.content[2], ContentBlock::Image(_)));
}

#[test]
fn openai_system_message() {
    let raw = json!({"role":"system","content":"sys"});
    let msg = normalize_message(raw, &Provider::OpenAI).unwrap();
    assert_eq!(msg.role, MessageRole::System);
}

#[test]
fn openai_denormalize_shape() {
    let msgs = vec![
        ConversationMessage::system("sys"),
        ConversationMessage::user(vec![ContentBlock::Text("hello".to_string())]),
        ConversationMessage::assistant(vec![ContentBlock::ToolCall(sample_tool_call())]),
        ConversationMessage {
            role: MessageRole::Tool,
            content: vec![ContentBlock::ToolResult(sample_tool_result())],
            stop_reason: None,
        },
    ];
    let wire = denormalize_messages(&msgs, &Provider::OpenAI).unwrap();
    assert!(wire["messages"].is_array());
    assert!(wire["system"].is_null());
}

#[test]
fn openai_missing_required_field() {
    let raw = json!({
        "role":"assistant",
        "tool_calls":[{"type":"function","function":{"name":"read_file","arguments":"{}"}}]
    });
    let err = normalize_message(raw, &Provider::OpenAI).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::MissingField { field: "id", provider: "OpenAI" }));
}

#[test]
fn openai_unknown_role() {
    let raw = json!({"role":"developer","content":"x"});
    let err = normalize_message(raw, &Provider::OpenAI).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnknownRole { provider: "OpenAI", .. }));
}

#[test]
fn gemini_plain_text_user() {
    let raw = json!({"role":"user","parts":[{"text":"hello"}]});
    let msg = normalize_message(raw, &Provider::Gemini).unwrap();
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content, vec![ContentBlock::Text("hello".to_string())]);
}

#[test]
fn gemini_model_role_and_stop_reason() {
    let raw = json!({
        "candidates":[{
            "content":{"role":"model","parts":[{"text":"done"}]},
            "finishReason":"STOP"
        }]
    });
    let msg = normalize_message(raw, &Provider::Gemini).unwrap();
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.stop_reason, Some(StopReason::EndTurn));
}

#[test]
fn gemini_tool_call_reasoning_and_inline_data() {
    let raw = json!({
        "role":"model",
        "parts":[
            {"thought":true,"text":"thinking","thoughtSignature":"sig"},
            {"functionCall":{"name":"read_file","args":{"path":"/tmp/a.txt"}}},
            {"inline_data":{"mime_type":"image/jpeg","data":"abc"}}
        ]
    });
    let msg = normalize_message(raw, &Provider::Gemini).unwrap();
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::Reasoning(_))));
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_))));
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::Image(_))));
}

#[test]
fn gemini_system_instruction() {
    let raw = json!({
        "system_instruction":{"parts":[{"text":"You are helpful"}]}
    });
    let msg = normalize_message(raw, &Provider::Gemini).unwrap();
    assert_eq!(msg.role, MessageRole::System);
}

#[test]
fn gemini_denormalize_shape() {
    let msgs = vec![
        ConversationMessage::system("sys"),
        ConversationMessage::user(vec![ContentBlock::Text("hello".to_string())]),
        ConversationMessage::assistant(vec![
            ContentBlock::Reasoning(ReasoningBlock::with_signature("thinking", "sig")),
            ContentBlock::ToolCall(sample_tool_call()),
            ContentBlock::Text("answer".to_string()),
        ]),
    ];
    let wire = denormalize_messages(&msgs, &Provider::Gemini).unwrap();
    assert!(wire["messages"].is_array());
    assert_eq!(wire["system"], "sys");
}

#[test]
fn gemini_missing_required_field() {
    let raw = json!({"role":"model","parts":[{"functionCall":{"args":{}}}]});
    let err = normalize_message(raw, &Provider::Gemini).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::MissingField { field: "functionCall.name", provider: "Gemini" }));
}

#[test]
fn gemini_unknown_role() {
    let raw = json!({"role":"assistant","parts":[{"text":"x"}]});
    let err = normalize_message(raw, &Provider::Gemini).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnknownRole { provider: "Gemini", .. }));
}

#[test]
fn deepseek_reasoning_content_extraction() {
    let raw = json!({
        "choices":[{"message":{"role":"assistant","content":"done","reasoning_content":"think"},"finish_reason":"stop"}]
    });
    let msg = normalize_message(raw, &Provider::DeepSeek).unwrap();
    assert!(matches!(msg.content[0], ContentBlock::Reasoning(_)));
    assert!(matches!(msg.content[1], ContentBlock::Text(_)));
}

#[test]
fn xai_reasoning_content_extraction() {
    let raw = json!({
        "choices":[{"message":{"role":"assistant","content":"done","reasoning_content":"think"},"finish_reason":"stop"}]
    });
    let msg = normalize_message(raw, &Provider::XAI).unwrap();
    assert!(matches!(msg.content[0], ContentBlock::Reasoning(_)));
}

#[test]
fn deepseek_and_xai_denormalize_include_reasoning_content() {
    let msgs = vec![ConversationMessage::assistant(vec![
        ContentBlock::Reasoning(ReasoningBlock::new("think")),
        ContentBlock::Text("answer".to_string()),
    ])];
    let ds = denormalize_messages(&msgs, &Provider::DeepSeek).unwrap();
    let xai = denormalize_messages(&msgs, &Provider::XAI).unwrap();
    assert!(ds["messages"][0].get("reasoning_content").is_some());
    assert!(xai["messages"][0].get("reasoning_content").is_some());
}

#[test]
fn deepseek_unknown_role() {
    let raw = json!({"role":"developer","content":"x"});
    let err = normalize_message(raw, &Provider::DeepSeek).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnknownRole { provider: "DeepSeek", .. }));
}

#[test]
fn xai_unknown_role() {
    let raw = json!({"role":"developer","content":"x"});
    let err = normalize_message(raw, &Provider::XAI).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnknownRole { provider: "xAI", .. }));
}

#[test]
fn groq_delegate_basic_paths() {
    let user = normalize_message(json!({"role":"user","content":"hello"}), &Provider::Groq).unwrap();
    assert_eq!(user.role, MessageRole::User);

    let assistant = normalize_message(json!({
        "choices":[{"message":{"role":"assistant","content":"done"},"finish_reason":"stop"}]
    }), &Provider::Groq).unwrap();
    assert_eq!(assistant.stop_reason, Some(StopReason::EndTurn));

    let wire = denormalize_messages(&[ConversationMessage::user(vec![ContentBlock::Text("x".into())])], &Provider::Groq).unwrap();
    assert!(wire["messages"].is_array());
}

#[test]
fn mistral_delegate_basic_paths() {
    let user = normalize_message(json!({"role":"user","content":"hello"}), &Provider::Mistral).unwrap();
    assert_eq!(user.role, MessageRole::User);

    let assistant = normalize_message(json!({
        "choices":[{"message":{"role":"assistant","content":"done"},"finish_reason":"length"}]
    }), &Provider::Mistral).unwrap();
    assert_eq!(assistant.stop_reason, Some(StopReason::MaxTokens));

    let wire = denormalize_messages(&[ConversationMessage::user(vec![ContentBlock::Text("x".into())])], &Provider::Mistral).unwrap();
    assert!(wire["messages"].is_array());
}

#[test]
fn openrouter_openai_and_anthropic_shape_detection() {
    let openai_shape = json!({
        "role":"assistant",
        "content":null,
        "tool_calls":[{"id":"call_123","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"/tmp/a.txt\"}"}}]
    });
    let msg = normalize_message(openai_shape, &Provider::OpenRouter).unwrap();
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_))));

    let anthropic_shape = json!({
        "role":"assistant",
        "content":[{"type":"tool_use","id":"toolu_1","name":"read_file","input":{"path":"/tmp/a.txt"}}]
    });
    let msg2 = normalize_message(anthropic_shape, &Provider::OpenRouter).unwrap();
    assert!(msg2.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_))));
}

#[test]
fn openrouter_openai_array_content_shape_detection() {
    let raw = json!({
        "role":"assistant",
        "content":[
            {"type":"text","text":"what is this"},
            {"type":"image_url","image_url":{"url":"https://example.com/a.jpg"}}
        ]
    });
    let msg = normalize_message(raw, &Provider::OpenRouter).unwrap();
    assert_eq!(msg.role, MessageRole::Assistant);
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::Text(_))));
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::Image(_))));
}

#[test]
fn openrouter_denormalize_openai_style() {
    let wire = denormalize_messages(
        &[ConversationMessage::assistant(vec![ContentBlock::ToolCall(sample_tool_call())])],
        &Provider::OpenRouter,
    )
    .unwrap();
    assert!(wire["messages"][0].get("tool_calls").is_some());
}

#[test]
fn openrouter_unknown_shape() {
    let err = normalize_message(json!({"weird":true}), &Provider::OpenRouter).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnknownShape { provider: "OpenRouter", .. }));
}

#[test]
fn ollama_native_plain_text_and_thinking_stop_reason() {
    let raw = json!({
        "message":{"role":"assistant","content":"done","thinking":"thinking"},
        "done_reason":"stop"
    });
    let msg = normalize_message(raw, &Provider::Ollama).unwrap();
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.stop_reason, Some(StopReason::EndTurn));
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::Reasoning(_))));
}

#[test]
fn ollama_openai_compat_detection() {
    let raw = json!({
        "choices":[{"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}]
    });
    let msg = normalize_message(raw, &Provider::Ollama).unwrap();
    assert_eq!(msg.stop_reason, Some(StopReason::EndTurn));
}

#[test]
fn ollama_tool_call_and_system_and_denormalize_native() {
    let raw = json!({
        "role":"assistant",
        "content":"x",
        "tool_calls":[{"id":"call_123","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"/tmp/a.txt\"}"}}]
    });
    let msg = normalize_message(raw, &Provider::Ollama).unwrap();
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_))));

    let sys = normalize_message(json!({"role":"system","content":"sys"}), &Provider::Ollama).unwrap();
    assert_eq!(sys.role, MessageRole::System);

    let native_wire = denormalize_messages(
        &[ConversationMessage::assistant(vec![
            ContentBlock::Reasoning(ReasoningBlock::new("think")),
            ContentBlock::Text("answer".to_string()),
        ])],
        &Provider::Ollama,
    )
    .unwrap();
    assert!(native_wire["messages"][0].get("thinking").is_some());
}

#[test]
fn ollama_missing_and_unknown_role() {
    let missing = normalize_message(json!({"content":"x"}), &Provider::Ollama).unwrap_err();
    assert!(matches!(missing, MessageNormalizeError::MissingField { field: "role", provider: "Ollama" }));

    let unknown = normalize_message(json!({"role":"developer","content":"x"}), &Provider::Ollama).unwrap_err();
    assert!(matches!(unknown, MessageNormalizeError::UnknownRole { provider: "Ollama", .. }));
}

#[test]
fn cohere_plain_user_and_assistant_stop_reason() {
    let user = normalize_message(json!({"role":"user","content":"hello"}), &Provider::Cohere).unwrap();
    assert_eq!(user.role, MessageRole::User);

    let assistant = normalize_message(
        json!({
            "message":{"role":"assistant","content":[{"type":"text","text":"done"}]},
            "finish_reason":"COMPLETE"
        }),
        &Provider::Cohere,
    )
    .unwrap();
    assert_eq!(assistant.stop_reason, Some(StopReason::EndTurn));
}

#[test]
fn cohere_tool_call_and_system_and_denormalize() {
    let raw = json!({
        "role":"assistant",
        "content":[{"type":"text","text":"hi"}],
        "tool_calls":[{"id":"call_123","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"/tmp/a.txt\"}"}}]
    });
    let msg = normalize_message(raw, &Provider::Cohere).unwrap();
    assert!(msg.content.iter().any(|b| matches!(b, ContentBlock::ToolCall(_))));

    let sys = normalize_message(json!({"role":"system","content":"sys"}), &Provider::Cohere).unwrap();
    assert_eq!(sys.role, MessageRole::System);

    let wire = denormalize_messages(
        &[
            ConversationMessage::user(vec![ContentBlock::Text("hello".to_string())]),
            ConversationMessage::assistant(vec![ContentBlock::ToolCall(sample_tool_call())]),
        ],
        &Provider::Cohere,
    )
    .unwrap();
    assert!(wire["messages"].is_array());
}

#[test]
fn cohere_missing_field_and_unknown_role() {
    let missing = normalize_message(json!({"content":"x"}), &Provider::Cohere).unwrap_err();
    assert!(matches!(missing, MessageNormalizeError::MissingField { field: "role", provider: "Cohere" }));

    let unknown = normalize_message(json!({"role":"developer","content":"x"}), &Provider::Cohere).unwrap_err();
    assert!(matches!(unknown, MessageNormalizeError::UnknownRole { provider: "Cohere", .. }));
}

#[test]
fn cohere_image_denormalize_unsupported() {
    let msgs = vec![ConversationMessage::user(vec![ContentBlock::Image(
        operon_context_normalize_messages::ImageBlock {
            source: operon_context_normalize_messages::ImageSource::Url("https://example.com/a.jpg".to_string()),
        },
    )])];
    let err = denormalize_messages(&msgs, &Provider::Cohere).unwrap_err();
    assert!(matches!(err, MessageNormalizeError::UnsupportedContentType { provider: "Cohere", .. }));
}
