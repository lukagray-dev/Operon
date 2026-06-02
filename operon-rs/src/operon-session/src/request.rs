// request.rs — Provider HTTP request body construction.
//
// This module is responsible for translating the canonical internal types
// (ConversationMessage, ToolDefinition) into the provider-specific JSON body
// that is sent to the LLM API.
//
// It is intentionally stateless and pure — no I/O, no async. It takes canonical
// types as input and returns a serde_json::Value. All side-effects live in http.rs.
//
// Provider-specific differences are handled here:
//   Anthropic  — top-level "system" field, "tools" array, "stream" bool
//   OpenAI-family — system is a message in the messages array, "tools" with "function" wrapper
//
// For now Anthropic is fully implemented; OpenAI-family uses a structural
// approximation that works for most providers.
//
// NOTE: Both operon_context_normalize_tools and operon_context_normalize_messages
// re-export `Provider` from `operon_providers`. They are the same type — no
// conversion function is needed. We import directly from operon_providers here.

use operon_context_normalize_messages::{denormalize_messages, ConversationMessage};
use operon_context_normalize_tools::{denormalize_definition, ToolDefinition};
use operon_providers::Provider;
use serde_json::{json, Value};

use crate::error::SessionError;

// ─────────────────────────────────────────────────────────────────────────────
// build_request
// ─────────────────────────────────────────────────────────────────────────────

/// Build the full JSON request body to POST to the provider's messages endpoint.
///
/// # Steps
///
/// 1. Denormalize canonical `ConversationMessage` list → provider wire format.
///    Returns `{ "messages": [...], "system": ... }` where `system` may be null.
/// 2. Denormalize canonical `ToolDefinition` list → provider wire format.
/// 3. Assemble the final body according to the provider's expected shape.
///
/// # Errors
///
/// Returns [`SessionError::Normalize`] if message or tool denormalization fails.
pub fn build_request(
    provider: &Provider,
    model_id: &str,
    max_tokens: usize,
    messages: &[ConversationMessage],
    tool_defs: &[ToolDefinition],
    stream: bool,
) -> Result<Value, SessionError> {
    // ── Step 1: Denormalize messages into provider wire format ────────────────
    // denormalize_messages returns { "messages": [...], "system": <value|null> }.
    // Both normalize-messages and normalize-tools re-export operon_providers::Provider,
    // so `provider` is accepted directly by both denormalize functions.
    let wire = denormalize_messages(messages, provider)
        .map_err(|e| SessionError::Normalize(e.to_string()))?;

    // Extract the two relevant fields from the wire envelope.
    let messages_arr = wire["messages"].clone();
    let system_val = wire["system"].clone();

    // ── Step 2: Denormalize tool definitions ──────────────────────────────────
    // Each ToolDefinition is converted independently. The provider enum selects
    // the output schema (Anthropic uses input_schema, OpenAI uses parameters, etc.)
    let wire_tools: Vec<Value> = tool_defs
        .iter()
        .map(|def| {
            denormalize_definition(def, provider)
                .map_err(|e| SessionError::Normalize(e.to_string()))
        })
        .collect::<Result<_, _>>()?;

    // ── Step 3: Assemble body per provider ───────────────────────────────────
    let body = match provider {
        // Anthropic has a dedicated top-level "system" field that is separate
        // from the messages array. We only include it if non-null.
        Provider::Anthropic => {
            let mut b = json!({
                "model":      model_id,
                "max_tokens": max_tokens,
                "messages":   messages_arr,
                "stream":     stream,
            });
            // Inject the system field only when the sanitizer actually produced one.
            if !system_val.is_null() {
                b["system"] = system_val;
            }
            // Inject tools only when tools are registered — avoids sending an
            // empty array which some providers interpret differently.
            if !wire_tools.is_empty() {
                b["tools"] = Value::Array(wire_tools);
            }
            b
        }

        // OpenAI-family providers (OpenAI, DeepSeek, OpenRouter, Groq, Mistral,
        // XAI, Ollama): the system message is already embedded inside messages_arr
        // by denormalize_messages. Gemini and Cohere also follow this fallthrough
        // with their own denormalization shapes.
        _ => {
            let mut b = json!({
                "model":      model_id,
                "max_tokens": max_tokens,
                "messages":   messages_arr,
                "stream":     stream,
            });
            if !wire_tools.is_empty() {
                b["tools"] = Value::Array(wire_tools);
            }
            b
        }
    };

    Ok(body)
}

// ─────────────────────────────────────────────────────────────────────────────
// provider_endpoint
// ─────────────────────────────────────────────────────────────────────────────

/// Return the default HTTPS endpoint URL for a given provider.
///
/// These are the canonical streaming-capable endpoints matching the defaults
/// in `operon_providers::ProviderCapabilities::default_base_url`.
///
/// # Note on custom base URLs
///
/// The runner uses `ProviderConfig::effective_base_url()` instead of this function
/// so that Ollama's custom port (or any other override) is respected.
/// This function is kept as a pure static lookup for reference and testing.
pub fn provider_endpoint(provider: &Provider) -> &'static str {
    match provider {
        Provider::Anthropic => "https://api.anthropic.com/v1/messages",
        Provider::OpenAI => "https://api.openai.com/v1/chat/completions",
        Provider::DeepSeek => "https://api.deepseek.com/v1/chat/completions",
        Provider::OpenRouter => "https://openrouter.ai/api/v1/chat/completions",
        Provider::Groq => "https://api.groq.com/openai/v1/chat/completions",
        Provider::Mistral => "https://api.mistral.ai/v1/chat/completions",
        Provider::XAI => "https://api.x.ai/v1/chat/completions",
        Provider::Ollama => "http://localhost:11434/v1/chat/completions",
        Provider::Gemini => "https://generativelanguage.googleapis.com/v1beta/models",
        Provider::Cohere => "https://api.cohere.com/v2/chat",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use operon_context_normalize_messages::{ContentBlock, ConversationMessage};
    use operon_context_normalize_tools::ToolDefinition;
    use operon_providers::Provider;
    use serde_json::json;

    /// Helper: build a simple user+assistant conversation for request tests.
    fn simple_messages() -> Vec<ConversationMessage> {
        vec![
            ConversationMessage::system("You are a helpful assistant."),
            ConversationMessage::user(vec![ContentBlock::Text("Hello!".to_string())]),
        ]
    }

    /// Helper: build a single no-arg tool definition for testing.
    fn simple_tool() -> ToolDefinition {
        ToolDefinition {
            name: "ping".to_string(),
            description: "Returns pong.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    #[test]
    fn anthropic_body_contains_required_top_level_fields() {
        // Anthropic requests must have model, max_tokens, messages, and stream.
        let msgs = simple_messages();
        let body = build_request(
            &Provider::Anthropic,
            "claude-sonnet-4-20250514",
            1024,
            &msgs,
            &[],
            true,
        )
        .expect("build_request should succeed");

        assert_eq!(body["model"], "claude-sonnet-4-20250514");
        assert_eq!(body["max_tokens"], 1024);
        assert_eq!(body["stream"], true);
        assert!(body["messages"].is_array());
    }

    #[test]
    fn anthropic_body_has_top_level_system_field() {
        // Anthropic uses a separate top-level "system" field, not a message in the array.
        let msgs = simple_messages();
        let body = build_request(
            &Provider::Anthropic,
            "claude-sonnet-4-20250514",
            1024,
            &msgs,
            &[],
            true,
        )
        .expect("build_request should succeed");

        // The system message should be extracted into the top-level "system" field.
        assert!(
            body.get("system").is_some(),
            "Anthropic body must have a top-level 'system' field"
        );
    }

    #[test]
    fn anthropic_body_includes_tools_when_provided() {
        // When tool definitions are supplied, they should appear in the body.
        let msgs = simple_messages();
        let tools = vec![simple_tool()];
        let body = build_request(
            &Provider::Anthropic,
            "claude-sonnet-4-20250514",
            1024,
            &msgs,
            &tools,
            true,
        )
        .expect("build_request should succeed");

        let tool_arr = body["tools"].as_array().expect("tools should be an array");
        assert_eq!(tool_arr.len(), 1);
        // Anthropic uses "input_schema" instead of "parameters".
        assert!(
            tool_arr[0].get("input_schema").is_some(),
            "Anthropic tool definition should use 'input_schema'"
        );
    }

    #[test]
    fn anthropic_body_omits_tools_when_empty() {
        // No tools registered → "tools" key should be absent entirely.
        let msgs = simple_messages();
        let body = build_request(
            &Provider::Anthropic,
            "claude-sonnet-4-20250514",
            1024,
            &msgs,
            &[],
            true,
        )
        .expect("build_request should succeed");

        assert!(
            body.get("tools").is_none(),
            "tools key should be absent when no tools are registered"
        );
    }

    #[test]
    fn stream_false_sets_stream_field_correctly() {
        // Non-streaming requests (e.g. the compaction client) should set stream=false.
        let msgs = simple_messages();
        let body = build_request(
            &Provider::Anthropic,
            "claude-sonnet-4-20250514",
            1024,
            &msgs,
            &[],
            false,
        )
        .expect("build_request should succeed");

        assert_eq!(body["stream"], false);
    }
}
