//! Integration tests for `operon-context-normalize-tools`.
//!
//! One test function per provider, each covering:
//!   1. Valid input — normalize a wire tool-call, verify canonical fields.
//!   2. Denormalize a ToolDefinition — verify provider-specific structure.
//!   3. Denormalize a ToolResult — verify provider-specific structure.
//!   4. Missing required field — verify `MissingField` error variant.
//!   5. Malformed arguments — verify `ArgumentParseFailed` (OpenAI-compatible providers)
//!      or shape-detection failure (OpenRouter) or schema conversion error (Cohere).
//!
//! All test fixtures use `serde_json::json!` — no string literals for JSON.

use operon_context_normalize_tools::{
    denormalize_definition, denormalize_result, normalize, Provider, ToolCall, ToolCallId,
    ToolContent, ToolDefinition, ToolNormalizeError, ToolResult,
};
use serde_json::json;

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers — build reusable fixtures to keep tests DRY
// ─────────────────────────────────────────────────────────────────────────────

/// A minimal but realistic ToolDefinition that all provider tests can reuse.
fn make_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "read_file".to_string(),
        description: "Read the contents of a file at the given absolute path.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                }
            },
            "required": ["path"]
        }),
    }
}

/// A minimal ToolResult for the text-content case.
fn make_tool_result(call_id: &str, name: &str) -> ToolResult {
    ToolResult {
        call_id: ToolCallId(call_id.to_string()),
        name: name.to_string(),
        content: ToolContent::Text("contents of /etc/hosts".to_string()),
        is_error: false,
    }
}

/// A ToolResult carrying JSON content — tests that JSON serialization works correctly.
fn make_json_tool_result(call_id: &str, name: &str) -> ToolResult {
    ToolResult {
        call_id: ToolCallId(call_id.to_string()),
        name: name.to_string(),
        content: ToolContent::Json(json!({ "lines": 3, "bytes": 42 })),
        is_error: false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Anthropic
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn anthropic_valid_normalize() {
    // Anthropic incoming tool call: "input" is a real JSON object, not a string
    let raw = json!({
        "type": "tool_use",
        "id": "toolu_01A02B03C",
        "name": "read_file",
        "input": { "path": "/etc/hosts" }
    });

    let call: ToolCall = normalize(raw, &Provider::Anthropic).unwrap();

    assert_eq!(call.id.0, "toolu_01A02B03C");
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments, json!({ "path": "/etc/hosts" }));
}

#[test]
fn anthropic_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::Anthropic).unwrap();

    // Anthropic uses "input_schema" not "parameters"
    assert_eq!(wire["name"], "read_file");
    assert!(
        wire.get("input_schema").is_some(),
        "Anthropic definition must have 'input_schema' key"
    );
    // The parameters schema is passed through unchanged
    assert_eq!(wire["input_schema"], def.parameters);
    // No "parameters" key — that's OpenAI-style
    assert!(wire.get("parameters").is_none());
}

#[test]
fn anthropic_denormalize_result_text() {
    let result = make_tool_result("toolu_01A02B03C", "read_file");
    let wire = denormalize_result(&result, &Provider::Anthropic).unwrap();

    assert_eq!(wire["type"], "tool_result");
    assert_eq!(wire["tool_use_id"], "toolu_01A02B03C");
    assert_eq!(wire["content"], "contents of /etc/hosts");
    assert_eq!(wire["is_error"], false);
}

#[test]
fn anthropic_denormalize_result_json_content() {
    let result = make_json_tool_result("toolu_01A02B03C", "read_file");
    let wire = denormalize_result(&result, &Provider::Anthropic).unwrap();

    assert_eq!(wire["type"], "tool_result");
    // JSON content is serialized to a compact string
    assert!(wire["content"].is_string());
}

#[test]
fn anthropic_missing_id() {
    // Missing the required "id" field
    let raw = json!({ "type": "tool_use", "name": "read_file", "input": {} });
    let err = normalize(raw, &Provider::Anthropic).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "Anthropic" }),
        "expected MissingField for 'id', got: {err}"
    );
}

#[test]
fn anthropic_missing_input() {
    // Missing the "input" field (Anthropic's equivalent of "arguments")
    let raw = json!({ "type": "tool_use", "id": "toolu_01", "name": "read_file" });
    let err = normalize(raw, &Provider::Anthropic).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "input", provider: "Anthropic" }),
        "expected MissingField for 'input', got: {err}"
    );
}

#[test]
fn anthropic_missing_name() {
    // Missing the "name" field
    let raw = json!({ "type": "tool_use", "id": "toolu_01", "input": {} });
    let err = normalize(raw, &Provider::Anthropic).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "name", provider: "Anthropic" }),
        "expected MissingField for 'name', got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAI
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn openai_valid_normalize() {
    // OpenAI: "arguments" is a JSON-encoded STRING
    let raw = json!({
        "id": "call_abc123",
        "type": "function",
        "function": {
            "name": "read_file",
            "arguments": "{\"path\":\"/etc/hosts\"}"
        }
    });

    let call = normalize(raw, &Provider::OpenAI).unwrap();

    assert_eq!(call.id.0, "call_abc123");
    assert_eq!(call.name, "read_file");
    // Arguments must be a parsed object, not the raw string
    assert_eq!(call.arguments, json!({ "path": "/etc/hosts" }));
    assert!(call.arguments.is_object(), "arguments must be a JSON object");
}

#[test]
fn openai_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::OpenAI).unwrap();

    // OpenAI wraps in { "type": "function", "function": { ... } }
    assert_eq!(wire["type"], "function");
    let function = wire.get("function").expect("must have 'function' key");
    assert_eq!(function["name"], "read_file");
    assert_eq!(function["description"], def.description);
    assert_eq!(function["parameters"], def.parameters);
}

#[test]
fn openai_denormalize_result() {
    let result = make_tool_result("call_abc123", "read_file");
    let wire = denormalize_result(&result, &Provider::OpenAI).unwrap();

    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["tool_call_id"], "call_abc123");
    assert_eq!(wire["name"], "read_file");
    assert_eq!(wire["content"], "contents of /etc/hosts");
}

#[test]
fn openai_missing_id() {
    let raw = json!({
        "type": "function",
        "function": { "name": "read_file", "arguments": "{}" }
    });
    let err = normalize(raw, &Provider::OpenAI).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "OpenAI" }),
        "expected MissingField for 'id', got: {err}"
    );
}

#[test]
fn openai_missing_function_key() {
    // The "function" wrapper is absent
    let raw = json!({ "id": "call_abc", "type": "function" });
    let err = normalize(raw, &Provider::OpenAI).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "function", provider: "OpenAI" }),
        "expected MissingField for 'function', got: {err}"
    );
}

#[test]
fn openai_malformed_arguments() {
    // "arguments" is a string but contains invalid JSON
    let raw = json!({
        "id": "call_abc",
        "type": "function",
        "function": {
            "name": "read_file",
            "arguments": "NOT VALID JSON {"
        }
    });
    let err = normalize(raw, &Provider::OpenAI).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "OpenAI", .. }),
        "expected ArgumentParseFailed, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Gemini
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn gemini_valid_normalize() {
    // Gemini: no "id" field — one must be generated synthetically
    let raw = json!({
        "functionCall": {
            "name": "read_file",
            "args": { "path": "/etc/hosts" }
        }
    });

    let call = normalize(raw, &Provider::Gemini).unwrap();

    // Name and arguments must be extracted correctly
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments, json!({ "path": "/etc/hosts" }));
    // The synthesized ID must start with the "gemini-" prefix
    assert!(
        call.id.0.starts_with("gemini-"),
        "Gemini ID must start with 'gemini-', got: {}",
        call.id.0
    );
    // The ID string after the prefix must be 16 hex characters
    let hex_part = call.id.0.strip_prefix("gemini-").unwrap();
    assert_eq!(hex_part.len(), 16, "hex part must be 16 chars, got: {hex_part}");
}

#[test]
fn gemini_id_is_deterministic() {
    // The same input must always produce the same ID (needed to pair call ↔ result)
    let raw = json!({
        "functionCall": { "name": "my_tool", "args": { "x": 1, "y": 2 } }
    });

    let call1 = normalize(raw.clone(), &Provider::Gemini).unwrap();
    let call2 = normalize(raw, &Provider::Gemini).unwrap();

    assert_eq!(call1.id, call2.id, "Gemini IDs must be deterministic for the same input");
}

#[test]
fn gemini_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::Gemini).unwrap();

    // Gemini wraps definitions in { "function_declarations": [...] }
    let decls = wire
        .get("function_declarations")
        .and_then(|v| v.as_array())
        .expect("Gemini definition must have 'function_declarations' array");

    assert_eq!(decls.len(), 1);
    assert_eq!(decls[0]["name"], "read_file");
    assert_eq!(decls[0]["parameters"], def.parameters);
}

#[test]
fn gemini_denormalize_result() {
    let result = make_tool_result("gemini-abc123", "read_file");
    let wire = denormalize_result(&result, &Provider::Gemini).unwrap();

    // Gemini tool results use "functionResponse" not "tool_result"
    let fr = wire
        .get("functionResponse")
        .expect("Gemini result must have 'functionResponse' key");

    assert_eq!(fr["name"], "read_file");
    assert!(fr.get("response").is_some(), "must have 'response' key");
    // Text content is wrapped in { "content": "..." }
    assert_eq!(fr["response"]["content"], "contents of /etc/hosts");
}

#[test]
fn gemini_missing_function_call() {
    // The "functionCall" wrapper is absent
    let raw = json!({ "name": "read_file", "args": {} });
    let err = normalize(raw, &Provider::Gemini).unwrap_err();

    assert!(
        matches!(
            err,
            ToolNormalizeError::MissingField { field: "functionCall", provider: "Gemini" }
        ),
        "expected MissingField for 'functionCall', got: {err}"
    );
}

#[test]
fn gemini_missing_args() {
    // "args" is absent from inside "functionCall"
    let raw = json!({ "functionCall": { "name": "read_file" } });
    let err = normalize(raw, &Provider::Gemini).unwrap_err();

    assert!(
        matches!(
            err,
            ToolNormalizeError::MissingField { field: "functionCall.args", provider: "Gemini" }
        ),
        "expected MissingField for 'functionCall.args', got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Ollama (OpenAI-compatible delegate)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn ollama_valid_normalize() {
    let raw = json!({
        "id": "call_ollama_01",
        "type": "function",
        "function": { "name": "search", "arguments": "{\"query\":\"rust borrow checker\"}" }
    });

    let call = normalize(raw, &Provider::Ollama).unwrap();
    assert_eq!(call.id.0, "call_ollama_01");
    assert_eq!(call.name, "search");
    assert_eq!(call.arguments["query"], "rust borrow checker");
}

#[test]
fn ollama_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::Ollama).unwrap();

    // Ollama mirrors the OpenAI envelope
    assert_eq!(wire["type"], "function");
    assert!(wire.get("function").is_some());
}

#[test]
fn ollama_denormalize_result() {
    let result = make_tool_result("call_ollama_01", "search");
    let wire = denormalize_result(&result, &Provider::Ollama).unwrap();

    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["tool_call_id"], "call_ollama_01");
}

#[test]
fn ollama_missing_id() {
    let raw = json!({
        "type": "function",
        "function": { "name": "search", "arguments": "{}" }
    });
    let err = normalize(raw, &Provider::Ollama).unwrap_err();
    // Error should mention "Ollama", not "OpenAI"
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "Ollama" }),
        "expected MissingField for Ollama, got: {err}"
    );
}

#[test]
fn ollama_malformed_arguments() {
    let raw = json!({
        "id": "call_ollama_01",
        "type": "function",
        "function": { "name": "search", "arguments": "{{broken" }
    });
    let err = normalize(raw, &Provider::Ollama).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "Ollama", .. }),
        "expected ArgumentParseFailed for Ollama, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// DeepSeek (OpenAI-compatible delegate)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn deepseek_valid_normalize() {
    let raw = json!({
        "id": "call_ds_42",
        "type": "function",
        "function": { "name": "calculator", "arguments": "{\"expr\":\"2+2\"}" }
    });

    let call = normalize(raw, &Provider::DeepSeek).unwrap();
    assert_eq!(call.id.0, "call_ds_42");
    assert_eq!(call.name, "calculator");
    assert_eq!(call.arguments["expr"], "2+2");
}

#[test]
fn deepseek_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::DeepSeek).unwrap();
    assert_eq!(wire["type"], "function");
}

#[test]
fn deepseek_denormalize_result() {
    let result = make_tool_result("call_ds_42", "calculator");
    let wire = denormalize_result(&result, &Provider::DeepSeek).unwrap();
    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["name"], "calculator");
}

#[test]
fn deepseek_missing_id() {
    let raw = json!({
        "type": "function",
        "function": { "name": "calculator", "arguments": "{}" }
    });
    let err = normalize(raw, &Provider::DeepSeek).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "DeepSeek" }),
        "expected MissingField for DeepSeek, got: {err}"
    );
}

#[test]
fn deepseek_malformed_arguments() {
    let raw = json!({
        "id": "call_ds_42",
        "type": "function",
        "function": { "name": "calculator", "arguments": "not json" }
    });
    let err = normalize(raw, &Provider::DeepSeek).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "DeepSeek", .. }),
        "expected ArgumentParseFailed for DeepSeek, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Groq (OpenAI-compatible delegate)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn groq_valid_normalize() {
    let raw = json!({
        "id": "call_groq_99",
        "type": "function",
        "function": { "name": "web_search", "arguments": "{\"q\":\"llama 3\"}" }
    });

    let call = normalize(raw, &Provider::Groq).unwrap();
    assert_eq!(call.id.0, "call_groq_99");
    assert_eq!(call.name, "web_search");
    assert_eq!(call.arguments["q"], "llama 3");
}

#[test]
fn groq_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::Groq).unwrap();
    assert_eq!(wire["type"], "function");
}

#[test]
fn groq_denormalize_result() {
    let result = make_tool_result("call_groq_99", "web_search");
    let wire = denormalize_result(&result, &Provider::Groq).unwrap();
    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["tool_call_id"], "call_groq_99");
}

#[test]
fn groq_missing_function_key() {
    let raw = json!({ "id": "call_groq_99", "type": "function" });
    let err = normalize(raw, &Provider::Groq).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "function", provider: "Groq" }),
        "expected MissingField for Groq 'function' key, got: {err}"
    );
}

#[test]
fn groq_malformed_arguments() {
    let raw = json!({
        "id": "call_groq_99",
        "type": "function",
        "function": { "name": "web_search", "arguments": "{ bad }" }
    });
    let err = normalize(raw, &Provider::Groq).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "Groq", .. }),
        "expected ArgumentParseFailed for Groq, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Mistral (OpenAI-compatible delegate)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn mistral_valid_normalize() {
    let raw = json!({
        "id": "call_mistral_7b",
        "type": "function",
        "function": { "name": "translate", "arguments": "{\"text\":\"hello\",\"lang\":\"fr\"}" }
    });

    let call = normalize(raw, &Provider::Mistral).unwrap();
    assert_eq!(call.id.0, "call_mistral_7b");
    assert_eq!(call.name, "translate");
    assert_eq!(call.arguments["lang"], "fr");
}

#[test]
fn mistral_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::Mistral).unwrap();
    assert_eq!(wire["type"], "function");
    assert_eq!(wire["function"]["name"], "read_file");
}

#[test]
fn mistral_denormalize_result() {
    let result = make_tool_result("call_mistral_7b", "translate");
    let wire = denormalize_result(&result, &Provider::Mistral).unwrap();
    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["name"], "translate");
}

#[test]
fn mistral_missing_id() {
    let raw = json!({
        "type": "function",
        "function": { "name": "translate", "arguments": "{}" }
    });
    let err = normalize(raw, &Provider::Mistral).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "Mistral" }),
        "expected MissingField for Mistral, got: {err}"
    );
}

#[test]
fn mistral_malformed_arguments() {
    let raw = json!({
        "id": "call_mistral_7b",
        "type": "function",
        "function": { "name": "translate", "arguments": "oops" }
    });
    let err = normalize(raw, &Provider::Mistral).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "Mistral", .. }),
        "expected ArgumentParseFailed for Mistral, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// xAI (OpenAI-compatible delegate)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn xai_valid_normalize() {
    let raw = json!({
        "id": "call_grok_01",
        "type": "function",
        "function": { "name": "image_gen", "arguments": "{\"prompt\":\"sunset\"}" }
    });

    let call = normalize(raw, &Provider::XAI).unwrap();
    assert_eq!(call.id.0, "call_grok_01");
    assert_eq!(call.name, "image_gen");
    assert_eq!(call.arguments["prompt"], "sunset");
}

#[test]
fn xai_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::XAI).unwrap();
    assert_eq!(wire["type"], "function");
}

#[test]
fn xai_denormalize_result() {
    let result = make_tool_result("call_grok_01", "image_gen");
    let wire = denormalize_result(&result, &Provider::XAI).unwrap();
    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["tool_call_id"], "call_grok_01");
}

#[test]
fn xai_missing_id() {
    let raw = json!({
        "type": "function",
        "function": { "name": "image_gen", "arguments": "{}" }
    });
    let err = normalize(raw, &Provider::XAI).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "xAI" }),
        "expected MissingField for xAI, got: {err}"
    );
}

#[test]
fn xai_malformed_arguments() {
    let raw = json!({
        "id": "call_grok_01",
        "type": "function",
        "function": { "name": "image_gen", "arguments": "<<<invalid>>>" }
    });
    let err = normalize(raw, &Provider::XAI).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "xAI", .. }),
        "expected ArgumentParseFailed for xAI, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenRouter (shape-detection dispatch)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn openrouter_openai_shape() {
    // OpenRouter forwarding an OpenAI-backed model response
    let raw = json!({
        "id": "call_or_openai",
        "type": "function",
        "function": { "name": "read_file", "arguments": "{\"path\":\"/tmp/x\"}" }
    });

    let call = normalize(raw, &Provider::OpenRouter).unwrap();
    assert_eq!(call.id.0, "call_or_openai");
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments["path"], "/tmp/x");
}

#[test]
fn openrouter_anthropic_shape() {
    // OpenRouter forwarding an Anthropic-backed model response
    let raw = json!({
        "type": "tool_use",
        "id": "toolu_via_openrouter",
        "name": "read_file",
        "input": { "path": "/tmp/x" }
    });

    let call = normalize(raw, &Provider::OpenRouter).unwrap();
    assert_eq!(call.id.0, "toolu_via_openrouter");
    assert_eq!(call.name, "read_file");
}

#[test]
fn openrouter_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::OpenRouter).unwrap();
    // OpenRouter accepts OpenAI-style tool definitions
    assert_eq!(wire["type"], "function");
}

#[test]
fn openrouter_denormalize_result() {
    let result = make_tool_result("call_or_openai", "read_file");
    let wire = denormalize_result(&result, &Provider::OpenRouter).unwrap();
    assert_eq!(wire["role"], "tool");
}

#[test]
fn openrouter_unknown_shape() {
    // Shape cannot be detected — neither "function" key nor "type":"tool_use"
    let raw = json!({ "some_key": "some_value", "other_key": 42 });
    let err = normalize(raw, &Provider::OpenRouter).unwrap_err();

    assert!(
        matches!(err, ToolNormalizeError::UnknownShape { provider: "OpenRouter", .. }),
        "expected UnknownShape for OpenRouter, got: {err}"
    );
}

#[test]
fn openrouter_openai_shape_malformed_arguments() {
    // Detected as OpenAI shape but arguments are bad JSON
    let raw = json!({
        "id": "call_or_01",
        "type": "function",
        "function": { "name": "tool", "arguments": "NOTJSON" }
    });
    let err = normalize(raw, &Provider::OpenRouter).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::ArgumentParseFailed { provider: "OpenRouter", .. }),
        "expected ArgumentParseFailed for OpenRouter, got: {err}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Cohere
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn cohere_valid_normalize() {
    // Cohere: "parameters" is a real JSON object, not a string
    let raw = json!({
        "id": "tool_call_id_1",
        "type": "tool_call",
        "tool_call": {
            "name": "read_file",
            "parameters": { "path": "/etc/hosts" }
        }
    });

    let call = normalize(raw, &Provider::Cohere).unwrap();
    assert_eq!(call.id.0, "tool_call_id_1");
    assert_eq!(call.name, "read_file");
    assert_eq!(call.arguments, json!({ "path": "/etc/hosts" }));
}

#[test]
fn cohere_denormalize_definition() {
    let def = make_tool_definition();
    let wire = denormalize_definition(&def, &Provider::Cohere).unwrap();

    // Cohere uses "parameter_definitions" not JSON Schema
    assert_eq!(wire["name"], "read_file");
    let param_defs = wire
        .get("parameter_definitions")
        .expect("must have 'parameter_definitions'");

    // "path" should be in the definitions, type mapped to "str", required = true
    let path_def = param_defs.get("path").expect("must have 'path' definition");
    assert_eq!(path_def["type"], "str");
    assert_eq!(path_def["required"], true);
    assert_eq!(path_def["description"], "Absolute path to the file to read");
}

#[test]
fn cohere_denormalize_definition_type_mapping() {
    // Test that all JSON Schema types map to the correct Cohere types
    let def = ToolDefinition {
        name: "multi_type_tool".to_string(),
        description: "Tool with many param types".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "str_field":  { "type": "string",  "description": "a string" },
                "int_field":  { "type": "integer", "description": "an integer" },
                "num_field":  { "type": "number",  "description": "a number" },
                "bool_field": { "type": "boolean", "description": "a bool" },
                "obj_field":  { "type": "object",  "description": "an object" },
                "arr_field":  { "type": "array",   "description": "an array" },
            },
            "required": ["str_field", "int_field"]
        }),
    };

    let wire = denormalize_definition(&def, &Provider::Cohere).unwrap();
    let pd = wire.get("parameter_definitions").unwrap();

    assert_eq!(pd["str_field"]["type"],  "str");
    assert_eq!(pd["int_field"]["type"],  "int");
    assert_eq!(pd["num_field"]["type"],  "float");
    assert_eq!(pd["bool_field"]["type"], "bool");
    assert_eq!(pd["obj_field"]["type"],  "dict");
    // "array" has no Cohere equivalent — falls back to "str"
    assert_eq!(pd["arr_field"]["type"],  "str");

    // Only the required fields should have required=true
    assert_eq!(pd["str_field"]["required"],  true);
    assert_eq!(pd["int_field"]["required"],  true);
    assert_eq!(pd["num_field"]["required"],  false);
}

#[test]
fn cohere_denormalize_result_text() {
    let result = make_tool_result("tool_call_id_1", "read_file");
    let wire = denormalize_result(&result, &Provider::Cohere).unwrap();

    assert_eq!(wire["role"], "tool");
    assert_eq!(wire["tool_call_id"], "tool_call_id_1");

    // Cohere uses an array of content blocks
    let content = wire["content"].as_array().expect("content must be an array");
    assert_eq!(content.len(), 1);
    assert_eq!(content[0]["type"], "text");
    assert_eq!(content[0]["text"], "contents of /etc/hosts");
}

#[test]
fn cohere_denormalize_result_json_content() {
    let result = make_json_tool_result("tool_call_id_2", "read_file");
    let wire = denormalize_result(&result, &Provider::Cohere).unwrap();

    let content = wire["content"].as_array().expect("content must be an array");
    assert_eq!(content[0]["type"], "text");
    // JSON content is serialized to a compact string
    assert!(content[0]["text"].is_string());
}

#[test]
fn cohere_missing_id() {
    // Missing the top-level "id" field
    let raw = json!({
        "type": "tool_call",
        "tool_call": { "name": "read_file", "parameters": {} }
    });
    let err = normalize(raw, &Provider::Cohere).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "id", provider: "Cohere" }),
        "expected MissingField for 'id', got: {err}"
    );
}

#[test]
fn cohere_missing_tool_call_key() {
    // The "tool_call" sub-object is absent
    let raw = json!({ "id": "tool_call_id_1", "type": "tool_call" });
    let err = normalize(raw, &Provider::Cohere).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::MissingField { field: "tool_call", provider: "Cohere" }),
        "expected MissingField for 'tool_call', got: {err}"
    );
}

#[test]
fn cohere_definition_without_properties_fails() {
    // A definition with no "properties" key cannot be converted to Cohere format
    let def = ToolDefinition {
        name: "bad_tool".to_string(),
        description: "No properties in schema".to_string(),
        // Missing the "properties" key — Cohere conversion requires it
        parameters: json!({ "type": "object" }),
    };
    let err = denormalize_definition(&def, &Provider::Cohere).unwrap_err();
    assert!(
        matches!(err, ToolNormalizeError::CohereSchemaConversion { .. }),
        "expected CohereSchemaConversion, got: {err}"
    );
}
