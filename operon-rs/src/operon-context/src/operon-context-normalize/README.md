# operon-context-normalize

Facade crate for Operon context normalization.

This crate has no normalization logic of its own. It only re-exports the four dedicated normalization crates so callers can use a single dependency when desired.

Sub-crates remain independently usable when a narrower dependency is preferred.

## Re-exported Modules

- `tools` -> `operon-context-normalize-tools`
  - canonical tool-call types and wire conversion
- `reasoning` -> `operon-context-normalize-reasoning`
  - canonical reasoning/thinking block conversion
- `messages` -> `operon-context-normalize-messages`
  - canonical conversation message conversion
- `stream` -> `operon-context-normalize-stream`
  - canonical stream-event parsing and stream assembly

## Flattened Root Exports

For convenience, common types are also re-exported at crate root:

- tools:
  - `Provider`
  - `ToolCall`
  - `ToolCallId`
  - `ToolContent`
  - `ToolDefinition`
  - `ToolResult`
- reasoning:
  - `ReasoningBlock`
  - `ReasoningSignature`
- messages:
  - `ConversationMessage`
  - `ContentBlock`
  - `MessageRole`
  - `StopReason`
- stream:
  - `StreamEvent`
  - `AssemblerOutput`

## Example

```rust
use operon_context_normalize::{
    Provider, ConversationMessage, ContentBlock, MessageRole, StreamEvent,
};

let _provider = Provider::OpenAI;
let msg = ConversationMessage {
    role: MessageRole::User,
    content: vec![ContentBlock::Text("hello".to_string())],
    stop_reason: None,
};
let _event = StreamEvent::TextDelta { text: "chunk".to_string() };
assert_eq!(msg.content.len(), 1);
```

## Build

```bash
cargo check -p operon-context-normalize
```
