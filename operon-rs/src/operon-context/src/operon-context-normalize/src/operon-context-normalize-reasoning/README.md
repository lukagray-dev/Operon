# operon-context-normalize-reasoning

Canonical reasoning/thinking normalization for Operon.

This crate converts provider-specific reasoning payloads into a single canonical type (`ReasoningBlock`) and converts canonical reasoning back to provider wire format.

The crate is pure conversion:
- no HTTP
- no async runtime
- no execution logic
- no persistence

## Core API

- `normalize_reasoning(raw: Value, provider: &Provider) -> Result<Vec<ReasoningBlock>>`
- `denormalize_reasoning(blocks: &[ReasoningBlock], provider: &Provider) -> Result<Value>`

## Canonical Types

- `ReasoningBlock`
  - `thinking: String`
  - `signature: Option<ReasoningSignature>`

- `ReasoningSignature`
  - opaque provider token that must be echoed back verbatim when required

Helper constructors:
- `ReasoningBlock::new(thinking)`
- `ReasoningBlock::with_signature(thinking, sig)`

## Supported Providers

- Anthropic
- OpenAI
- Gemini
- Ollama
- DeepSeek
- OpenRouter
- Groq (not supported for reasoning payloads)
- Mistral (not supported for reasoning payloads)
- XAI
- Cohere (not supported for reasoning payloads)

## Provider Notes

- Anthropic: `type=thinking`, `thinking`, optional `signature`.
- OpenAI: `reasoning_summary` array of `summary_text` objects.
- Gemini: thought parts with `thought=true`, `text`, optional `thoughtSignature`.
- DeepSeek / XAI: plain string reasoning content field.
- Ollama: plain string thinking field in native path.
- OpenRouter: shape-detects string/array/object and delegates behavior.

For providers that do not expose reasoning publicly (Groq, Mistral, Cohere), APIs return `ReasoningNormalizeError::NotSupported`.

## Error Model

Single error type: `ReasoningNormalizeError`
- `MissingField`
- `NotSupported`
- `SerializeFailed`
- `EmptyReasoningSummary`
- `UnknownShape`

## Example

```rust
use operon_context_normalize_reasoning::{
    normalize_reasoning, denormalize_reasoning, Provider, ReasoningBlock,
};
use serde_json::json;

let raw = json!({
    "type": "thinking",
    "thinking": "Break down the problem.",
    "signature": "sig_123"
});

let blocks = normalize_reasoning(raw, &Provider::Anthropic).unwrap();
assert_eq!(blocks[0].thinking, "Break down the problem.");
assert!(blocks[0].has_signature());

let wire = denormalize_reasoning(&blocks, &Provider::Anthropic).unwrap();
assert!(wire.is_array());

let ds_wire = denormalize_reasoning(&[ReasoningBlock::new("step 1\nstep 2")], &Provider::DeepSeek).unwrap();
assert!(ds_wire.is_string());
```

## Testing

Integration tests:
- `tests/normalize_reasoning.rs`

Run:

```bash
cargo check -p operon-context-normalize-reasoning
cargo test -p operon-context-normalize-reasoning
```
