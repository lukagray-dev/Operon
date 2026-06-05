# operon-context-normalize-stream

Canonical streaming normalization for Operon.

This crate parses provider stream payload lines (SSE `data:` payloads or NDJSON lines) into provider-agnostic `StreamEvent` values, then assembles those events into complete outputs with `StreamAssembler`.

The crate is sync-only and push-based:
- no HTTP
- no async runtime
- no I/O
- no network dependencies

## Core API

- `parse_line(line: &str, provider: &Provider) -> Result<Vec<StreamEvent>>`
- `new_assembler(provider: &Provider) -> StreamAssembler`
- `StreamAssembler::push(event: StreamEvent) -> Result<AssemblerOutput>`
- `StreamAssembler::finish() -> Result<Vec<AssemblerOutput>>`

`parse_line`:
- accepts one already-split payload line
- ignores empty/comment/`[DONE]` frames
- can emit zero, one, or multiple events from a single line

`StreamAssembler`:
- buffers fragmented tool-call arguments by index
- emits text immediately
- emits completed tool calls when arguments become parseable JSON
- normalizes stop reason at stream end via `operon-context-normalize-messages`

## Canonical Types

- `StreamEvent`
  - `TextDelta`
  - `ReasoningDelta`
  - `ReasoningSignature`
  - `ToolCallStart`
  - `ToolCallDelta`
  - `ToolCallEnd`
  - `ToolCallComplete`
  - `StopReason`
  - `UsageMeta`
  - `StreamStart`
  - `Ping`

- `AssemblerOutput`
  - `Text`
  - `Reasoning`
  - `ToolCall`
  - `StreamEnded`
  - `Pending`

## Provider Model

This crate re-uses `Provider` from `operon-context-normalize-tools`, so callers use one shared provider enum across normalize crates.

Supported providers:
- Anthropic
- OpenAI
- Gemini
- Ollama
- DeepSeek
- OpenRouter
- Groq
- Mistral
- XAI
- Cohere

## Behavior Summary

- Anthropic: parses `message_start`, content-block deltas, thinking/signature deltas, and stop reason.
- OpenAI family: parses text deltas, `tool_calls` fragments, optional `reasoning_content`, finish reason, usage chunks.
- Gemini: parses `parts` arrays (`text`, `thought`, `thoughtSignature`, `functionCall`) and finish reason.
- Ollama: auto-detects OpenAI-compatible `/v1` vs native `/api/chat` NDJSON.
- Cohere: parses typed event stream (`content-delta`, tool-call start/delta/end, message-end).
- OpenRouter: shape-detects and delegates to OpenAI or Anthropic parser.

## Error Model

Single error type: `StreamNormalizeError`
- `MalformedJson`
- `MissingField`
- `UnknownEventType`
- `ToolArgsParseFailed`
- `AssemblerIncomplete`

## Example

```rust
use operon_context_normalize_stream::{
    parse_line, new_assembler, AssemblerOutput, Provider,
};

let provider = Provider::OpenAI;
let mut assembler = new_assembler(&provider);

let lines = [
    r#"{"choices":[{"delta":{"content":"Hello "},"finish_reason":null}]}"#,
    r#"{"choices":[{"delta":{"content":"world"},"finish_reason":"stop"}]}"#,
];

for line in lines {
    for event in parse_line(line, &provider).unwrap() {
        match assembler.push(event).unwrap() {
            AssemblerOutput::Text(text) => {
                assert!(!text.is_empty());
            }
            _ => {}
        }
    }
}

let final_outputs = assembler.finish().unwrap();
assert!(matches!(final_outputs[0], AssemblerOutput::StreamEnded { .. }));
```

## Testing

Integration tests:
- `tests/normalize_stream.rs`

Run:

```bash
cargo check -p operon-context-normalize-stream
cargo test -p operon-context-normalize-stream
```
