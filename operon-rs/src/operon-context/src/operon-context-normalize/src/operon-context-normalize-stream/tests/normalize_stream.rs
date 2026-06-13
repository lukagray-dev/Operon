//! Integration tests for `operon-context-normalize-stream`.

use operon_context_normalize_messages::StopReason;
use operon_context_normalize_stream::{
    new_assembler, parse_line, AssemblerOutput, Provider, StreamEvent, StreamNormalizeError,
};
use serde_json::json;

fn assert_single_event(line: &str, provider: &Provider, expected: StreamEvent) {
    let events = parse_line(line, provider).unwrap();
    assert_eq!(events, vec![expected]);
}

#[test]
fn parse_line_empty_comment_and_done_are_ignored_for_all_providers() {
    let providers = vec![
        Provider::Anthropic,
        Provider::OpenAI,
        Provider::Gemini,
        Provider::Ollama,
        Provider::DeepSeek,
        Provider::OpenRouter,
        Provider::Groq,
        Provider::Mistral,
        Provider::XAI,
        Provider::NvidiaNim,
        Provider::Cohere,
    ];

    for provider in providers {
        assert!(parse_line("", &provider).unwrap().is_empty());
        assert!(parse_line("   ", &provider).unwrap().is_empty());
        assert!(parse_line("[DONE]", &provider).unwrap().is_empty());
        assert!(parse_line(": ping", &provider).unwrap().is_empty());
    }
}

#[test]
fn anthropic_parse_text_stop_and_malformed() {
    assert_single_event(
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
        &Provider::Anthropic,
        StreamEvent::TextDelta {
            text: "Hello".to_string(),
        },
    );

    assert_single_event(
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#,
        &Provider::Anthropic,
        StreamEvent::StopReason {
            raw: "end_turn".to_string(),
        },
    );

    let err = parse_line("{bad json", &Provider::Anthropic).unwrap_err();
    assert!(matches!(
        err,
        StreamNormalizeError::MalformedJson {
            provider: "Anthropic",
            ..
        }
    ));
}

#[test]
fn anthropic_thinking_and_signature_finish_to_reasoning() {
    let mut assembler = new_assembler(&Provider::Anthropic);

    let reasoning_events = parse_line(
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"step by step"}}"#,
        &Provider::Anthropic,
    )
    .unwrap();
    assert_eq!(reasoning_events.len(), 1);
    assert_eq!(
        assembler.push(reasoning_events[0].clone()).unwrap(),
        AssemblerOutput::ReasoningDelta("step by step".to_string())
    );

    let signature_events = parse_line(
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig_1"}}"#,
        &Provider::Anthropic,
    )
    .unwrap();
    assert!(matches!(
        assembler.push(signature_events[0].clone()).unwrap(),
        AssemblerOutput::Pending
    ));

    // Finish assembly and verify it yields both our reasoning block and a default stream end output
    let finished = assembler.finish().unwrap();
    assert_eq!(
        finished,
        vec![
            AssemblerOutput::Reasoning {
                text: "step by step".to_string(),
                signature: Some("sig_1".to_string()),
            },
            AssemblerOutput::StreamEnded { stop_reason: None }
        ]
    );
}

#[test]
fn openai_parse_text_stop_and_malformed() {
    assert_single_event(
        r#"{"choices":[{"delta":{"content":"Hello"},"finish_reason":null}]}"#,
        &Provider::OpenAI,
        StreamEvent::TextDelta {
            text: "Hello".to_string(),
        },
    );

    assert_single_event(
        r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        &Provider::OpenAI,
        StreamEvent::StopReason {
            raw: "stop".to_string(),
        },
    );

    let err = parse_line("{bad json", &Provider::OpenAI).unwrap_err();
    assert!(matches!(
        err,
        StreamNormalizeError::MalformedJson {
            provider: "OpenAI",
            ..
        }
    ));
}

#[test]
fn gemini_parse_text_stop_reason_and_usage() {
    let line = json!({
        "candidates": [{
            "content": {
                "role": "model",
                "parts": [
                    {"text": "hello"}
                ]
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {"promptTokenCount": 10}
    })
    .to_string();

    let events = parse_line(&line, &Provider::Gemini).unwrap();
    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta {
                text: "hello".to_string(),
            },
            StreamEvent::StopReason {
                raw: "STOP".to_string(),
            },
            StreamEvent::UsageMeta {
                raw: json!({"promptTokenCount":10}),
            },
        ]
    );
}

#[test]
fn ollama_native_text_is_parsed() {
    assert_single_event(
        r#"{"model":"qwen3","message":{"role":"assistant","content":"Hello"},"done":false}"#,
        &Provider::Ollama,
        StreamEvent::TextDelta {
            text: "Hello".to_string(),
        },
    );
}

#[test]
fn deepseek_reasoning_content_delta_is_exposed() {
    let events = parse_line(
        r#"{"choices":[{"delta":{"reasoning_content":"think","content":null},"finish_reason":null}]}"#,
        &Provider::DeepSeek,
    )
    .unwrap();

    assert_eq!(
        events,
        vec![StreamEvent::ReasoningDelta {
            text: "think".to_string(),
        }]
    );
}

#[test]
fn cohere_content_and_message_end_are_parsed() {
    assert_single_event(
        r#"{"type":"content-delta","delta":{"message":{"content":{"text":"Hello"}}}}"#,
        &Provider::Cohere,
        StreamEvent::TextDelta {
            text: "Hello".to_string(),
        },
    );

    let message_end_events = parse_line(
        r#"{"type":"message-end","delta":{"finish_reason":"COMPLETE","usage":{"input_tokens":10}}}"#,
        &Provider::Cohere,
    )
    .unwrap();
    assert_eq!(
        message_end_events,
        vec![
            StreamEvent::StopReason {
                raw: "COMPLETE".to_string(),
            },
            StreamEvent::UsageMeta {
                raw: json!({"input_tokens":10}),
            },
        ]
    );
}

#[test]
fn openrouter_delegates_openai_shape() {
    let events = parse_line(
        r#"{"choices":[{"delta":{"content":"hello"},"finish_reason":"stop"}]}"#,
        &Provider::OpenRouter,
    )
    .unwrap();
    assert_eq!(
        events,
        vec![
            StreamEvent::TextDelta {
                text: "hello".to_string(),
            },
            StreamEvent::StopReason {
                raw: "stop".to_string(),
            },
        ]
    );
}

#[test]
fn groq_mistral_xai_nvidia_nim_delegate_to_openai_parser() {
    let line = r#"{"choices":[{"delta":{"content":"hi"},"finish_reason":"stop"}]}"#;

    let groq = parse_line(line, &Provider::Groq).unwrap();
    let mistral = parse_line(line, &Provider::Mistral).unwrap();
    let xai = parse_line(line, &Provider::XAI).unwrap();
    let nvidia_nim = parse_line(line, &Provider::NvidiaNim).unwrap();

    let expected = vec![
        StreamEvent::TextDelta {
            text: "hi".to_string(),
        },
        StreamEvent::StopReason {
            raw: "stop".to_string(),
        },
    ];
    assert_eq!(groq, expected);
    assert_eq!(mistral, expected);
    assert_eq!(xai, expected);
    assert_eq!(nvidia_nim, expected);
}

#[test]
fn assembler_round_trip_text_then_stop_reason() {
    let mut assembler = new_assembler(&Provider::OpenAI);
    let lines = [
        r#"{"choices":[{"delta":{"content":"Hello "},"finish_reason":null}]}"#,
        r#"{"choices":[{"delta":{"content":"world"},"finish_reason":"stop"}]}"#,
    ];

    let mut outputs = Vec::new();
    for line in lines {
        let events = parse_line(line, &Provider::OpenAI).unwrap();
        for event in events {
            outputs.push(assembler.push(event).unwrap());
        }
    }

    assert_eq!(
        outputs,
        vec![
            AssemblerOutput::Text("Hello ".to_string()),
            AssemblerOutput::Text("world".to_string()),
            AssemblerOutput::Pending,
        ]
    );

    // Verify that the final outputs from finish include the StreamEnded event
    let final_output = assembler.finish().unwrap();
    assert_eq!(
        final_output,
        vec![AssemblerOutput::StreamEnded {
            stop_reason: Some(StopReason::EndTurn),
        }]
    );
}
