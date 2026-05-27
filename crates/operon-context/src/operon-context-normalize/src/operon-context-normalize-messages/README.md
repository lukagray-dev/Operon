# operon-context-normalize-messages

Canonical conversation message normalization for the Operon runtime.

## Purpose

This crate converts provider-specific conversation message wire formats into one
canonical internal type, and converts canonical messages back into provider
wire format.

It is designed as a pure conversion layer:

- no HTTP calls
- no runtime execution
- no persistence
- no async

## Core API

- `normalize_message(raw: Value, provider: &Provider) -> Result<ConversationMessage>`
- `denormalize_messages(msgs: &[ConversationMessage], provider: &Provider) -> Result<Value>`

`denormalize_messages` always returns a JSON object with:

- `"messages"`: provider-formatted message array
- `"system"`: extracted/serialized system instruction when applicable (`null` when absent)

## Canonical Types

Main canonical model:

- `ConversationMessage`
  - `role: MessageRole`
  - `content: Vec<ContentBlock>`
  - `stop_reason: Option<StopReason>`

Block-level canonical content:

- `ContentBlock::Text`
- `ContentBlock::Image`
- `ContentBlock::Document`
- `ContentBlock::ToolCall`
- `ContentBlock::ToolResult`
- `ContentBlock::Reasoning`

Tool and reasoning blocks delegate to sibling crates:

- `operon-context-normalize-tools`
- `operon-context-normalize-reasoning`

## Supported Providers

`Provider` includes:

- `Anthropic`
- `OpenAI`
- `Gemini`
- `Ollama`
- `DeepSeek`
- `OpenRouter`
- `Groq`
- `Mistral`
- `XAI`
- `Cohere`

## Provider Behavior Notes

- `Anthropic`
  - Supports typed `content` blocks (`text`, `image`, `document`, `tool_use`, `tool_result`, `thinking`).
  - Supports top-level `system` normalization.

- `OpenAI` family (`OpenAI`, `Groq`, `Mistral`)
  - Shared OpenAI-compatible path.
  - Handles `tool_calls`, `role=tool`, `image_url`, and finish reasons.

- `DeepSeek` and `XAI`
  - OpenAI-compatible plus `reasoning_content` side field on assistant messages.

- `Gemini`
  - Handles `contents[].parts[]`, `role=model`, `functionCall`, `functionResponse`,
    `inline_data`, `file_data`, and thought parts.

- `Ollama`
  - Auto-detects OpenAI-compatible `/v1` shape vs native `/api/chat` shape.
  - Native path supports `thinking` and `done_reason`.

- `OpenRouter`
  - Shape-detects OpenAI-style vs Anthropic-style payloads on normalize.
  - Emits OpenAI-style wire shape on denormalize.

- `Cohere`
  - Supports v2 role/content patterns and tool calls/results.
  - Rejects image content on denormalize as unsupported.

## Stop Reason Normalization

`StopReason` is provider-agnostic and mapped from provider-specific raw values.
It supports known variants plus `Other(String)` for unmapped values.

## Error Model

Single error type: `MessageNormalizeError`

Variants:

- `MissingField`
- `UnknownRole`
- `UnknownShape`
- `SerializeFailed`
- `UnsupportedContentType`

## Testing

Integration tests are in:

- `tests/normalize_messages.rs`

Coverage includes:

- valid normalize/denormalize flows for all providers
- stop reason mapping in realistic envelopes
- tool call/tool result and reasoning handling
- OpenRouter shape detection cases
- error paths (`MissingField`, `UnknownRole`, `UnknownShape`, unsupported blocks)
