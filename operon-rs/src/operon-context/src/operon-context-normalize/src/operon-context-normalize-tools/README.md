# operon-context-normalize-tools

`operon-context-normalize-tools` is a high-performance, production-grade Rust crate designed to perform bidirectional normalization and denormalization of LLM tool-calling wire formats across ten major providers.

This crate serves as a leaf component with zero internal dependencies on other `operon` packages. It provides pure, type-safe, and deterministic parsing and serialization primitives that bridge the gap between provider-specific JSON wire shapes and a unified canonical representation.

```
                  ┌──────────────────────┐
                  │  Provider JSON Wire  │
                  └──────────┬───────────┘
                             │
            normalize()      │      denormalize_definition()
            (Wire -> Type)   │      denormalize_result()
                             ▼      (Type -> Wire)
                  ┌──────────────────────┐
                  │   Canonical Types    │
                  │   (ToolCall, etc.)   │
                  └──────────────────────┘
```

---

## Key Features

- **Bidirectional Conversions**: Standardize raw model tool-calls into typed Rust structures, and serialize tool definitions or execution results back into provider wire-conforming JSON.
- **Unified Arguments Parsing**: Auto-decodes JSON-encoded string arguments (such as those returned by OpenAI and compatible APIs) directly into structured, nested `serde_json::Value` objects inside the canonical `ToolCall`.
- **Robust Field Validation**: Rejects malformed JSON payloads, missing parameters, or incorrect structures on ingest, bubble-up errors early using standard Rust `Result` patterns.
- **Provider Support Matrix**:
  - **OpenAI Family**: OpenAI, DeepSeek, Groq, Mistral, Ollama, xAI
  - **Anthropic Family**: Anthropic Messages API
  - **Google Gemini**: Gemini GenerateContent API (with deterministic synthetic ID generation)
  - **Cohere**: Cohere Chat API (translates complex JSON schemas into Cohere's flat parameter definitions)
  - **OpenRouter**: Intelligent shape auto-detection mapping to either Anthropic or OpenAI styles at runtime

---

## Canonical Types

The crate exposes a minimal, highly cohesion-focused set of canonical models:

- **[`ToolCall`](file:///D:/Project%20Operon/Operon/crates/operon-context/src/operon-context-normalize/src/operon-context-normalize-tools/src/types.rs#L48-L84)**: Emitted by LLMs when calling a function. Consists of a unique `ToolCallId`, a `name`, and parsed JSON `arguments`.
- **[`ToolResult`](file:///D:/Project%20Operon/Operon/crates/operon-context/src/operon-context-normalize/src/operon-context-normalize-tools/src/types.rs#L89-L128)**: Wraps execution results, tracking `call_id` and whether the tool output represents an error (`is_error`).
- **[`ToolContent`](file:///D:/Project%20Operon/Operon/crates/operon-context/src/operon-context-normalize/src/operon-context-normalize-tools/src/types.rs#L130-L155)**: A type-safe representation of tool output, supporting either raw UTF-8 `Text` or nested structured `Json`.
- **[`ToolDefinition`](file:///D:/Project%20Operon/Operon/crates/operon-context/src/operon-context-normalize/src/operon-context-normalize-tools/src/types.rs#L160-L212)**: Describes the tools to present to the model. Employs standard JSON Schema representation for parameters.

---

## API Reference & Quick Start

Add this to your `Cargo.toml`:
```toml
[dependencies]
operon-context-normalize-tools = { path = "path/to/operon-context-normalize-tools" }
```

### Basic Example: Definition -> Ingestion -> Result Roundtrip

```rust
use operon_context_normalize_tools::{
    normalize, denormalize_definition, denormalize_result,
    Provider, ToolDefinition, ToolResult, ToolContent,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Declare and serialize a tool to be fed to Anthropic
    let definition = ToolDefinition {
        name: "calculate_tax".to_string(),
        description: "Calculate sales tax on a purchase amount".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "amount": { "type": "number", "description": "The subtotal amount" }
            },
            "required": ["amount"]
        }),
    };

    // Serializes to Anthropic's "input_schema" shape
    let anthropic_wire_def = denormalize_definition(&definition, &Provider::Anthropic)?;
    assert!(anthropic_wire_def.get("input_schema").is_some());

    // 2. Normalize a tool call received from OpenAI
    let openai_wire_call = json!({
        "id": "call_abc123",
        "type": "function",
        "function": {
            "name": "calculate_tax",
            "arguments": "{\"amount\": 150.0}" // OpenAI encodes arguments as a string
        }
    });

    let canonical_call = normalize(openai_wire_call, &Provider::OpenAI)?;
    assert_eq!(canonical_call.name, "calculate_tax");
    assert_eq!(canonical_call.arguments["amount"], 150.0); // Now a fully-parsed JSON number

    // 3. Serialize the execution result for OpenAI
    let result = ToolResult {
        call_id: canonical_call.id.clone(),
        name: canonical_call.name.clone(),
        content: ToolContent::Json(json!({ "tax": 12.0, "total": 162.0 })),
        is_error: false,
    };

    let openai_wire_result = denormalize_result(&result, &Provider::OpenAI)?;
    assert_eq!(openai_wire_result["role"], "tool");
    assert_eq!(openai_wire_result["tool_call_id"], "call_abc123");

    Ok(())
}
```

---

## Wire Translation Details

Each provider wire-format has minor, subtle differences. `operon-context-normalize-tools` abstracts these automatically:

### 1. Gemini GenerateContent API
- **Tool-Call ID**: Gemini does not provide a tool-call ID on the wire. This crate generates a deterministic, unique UUID based on the function call name and parameter fields, which matches the schema for referencing the call-id in the subsequent tool result request.
- **Conversion**:
  - `normalize` parses `functionCall` items.
  - `denormalize_result` formats results into a structured `functionResponse` block inside a user-role part.

### 2. Cohere Chat API
- **Parameters**: Cohere expects flat declarations under `parameter_definitions`. The crate recursively flattens the standard JSON Schema `properties` map, translating them into Cohere parameter metadata.
- **Results**: Cohere results are wrapped in a list under the `tool_results` key where the result itself is placed in an array under `outputs`.

### 3. OpenRouter Auto-Detection
- OpenRouter aggregates multiple backend models and sometimes returns tool-calls using either Anthropic's style (a block with a `type: "tool_use"`) or OpenAI's style (a list containing `type: "function"`). 
- The crate's `normalize` entry point parses the structure dynamically to identify the backend model format.

---

## Building and Testing

Ensure the workspace is built and tests run cleanly:

```bash
# Build the crate
cargo build --manifest-path crates/operon-context/src/operon-context-normalize/src/operon-context-normalize-tools/Cargo.toml

# Run the comprehensive integration test suite
cargo test --manifest-path crates/operon-context/src/operon-context-normalize/src/operon-context-normalize-tools/Cargo.toml
```
