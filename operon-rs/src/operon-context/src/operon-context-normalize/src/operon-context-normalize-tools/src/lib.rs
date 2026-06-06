//! # operon-context-normalize-tools
//!
//! Canonical tool-call types and bidirectional wire format conversion for eleven
//! major LLM providers.
//!
//! ## What this crate does
//!
//! Exactly one job:
//!
//! ```text
//! provider wire JSON  →  canonical internal types  →  provider wire JSON
//! ```
//!
//! No HTTP, no execution, no registry, no async, no I/O. Pure types and conversion.
//!
//! ## Supported providers
//!
//! | Provider | Wire format family |
//! |---|---|
//! | [`Provider::Anthropic`] | Anthropic Messages API |
//! | [`Provider::OpenAI`] | OpenAI Chat Completions API |
//! | [`Provider::Gemini`] | Google Gemini GenerateContent API |
//! | [`Provider::Ollama`] | OpenAI-compatible |
//! | [`Provider::DeepSeek`] | OpenAI-compatible |
//! | [`Provider::OpenRouter`] | Auto-detects OpenAI or Anthropic shape |
//! | [`Provider::Groq`] | OpenAI-compatible |
//! | [`Provider::Mistral`] | OpenAI-compatible |
//! | [`Provider::XAI`] | OpenAI-compatible |
//! | [`Provider::NvidiaNim`] | OpenAI-compatible |
//! | [`Provider::Cohere`] | Cohere Chat API (distinct shape) |
//!
//! ## Quick start
//!
//! ```rust
//! use operon_context_normalize_tools::{
//!     normalize, denormalize_definition, denormalize_result,
//!     Provider, ToolDefinition, ToolResult, ToolCallId, ToolContent,
//! };
//! use serde_json::json;
//!
//! // 1. Tell the model what tools it can use
//! let def = ToolDefinition {
//!     name: "read_file".to_string(),
//!     description: "Read the contents of a file.".to_string(),
//!     parameters: json!({
//!         "type": "object",
//!         "properties": { "path": { "type": "string" } },
//!         "required": ["path"]
//!     }),
//! };
//! let wire_def = denormalize_definition(&def, &Provider::OpenAI).unwrap();
//!
//! // 2. Parse the tool call the model emits
//! let wire_call = json!({
//!     "id": "call_abc",
//!     "type": "function",
//!     "function": { "name": "read_file", "arguments": "{\"path\":\"/etc/hosts\"}" }
//! });
//! let call = normalize(wire_call, &Provider::OpenAI).unwrap();
//! assert_eq!(call.name, "read_file");
//!
//! // 3. Run your tool, then feed the result back
//! let result = ToolResult {
//!     call_id: call.id.clone(),
//!     name: call.name.clone(),
//!     content: ToolContent::Text("127.0.0.1 localhost".to_string()),
//!     is_error: false,
//! };
//! let wire_result = denormalize_result(&result, &Provider::OpenAI).unwrap();
//! assert_eq!(wire_result["role"], "tool");
//! ```
//!
//! ## Standalone use
//!
//! This is a leaf crate with no `operon-*` dependencies. It can be used directly
//! in any Rust project that needs to normalize LLM tool-call wire formats.

// Declare all modules — each corresponds to a source file under src/
pub mod error;
pub mod normalize;
pub mod provider;
pub mod types;

// ─────────────────────────────────────────────────────────────────────────────
// Public re-exports — this is the exact API surface the crate exposes.
// External users should only need to import from the crate root, not sub-modules.
// ─────────────────────────────────────────────────────────────────────────────

/// The single error type for all normalization and denormalization operations.
pub use error::ToolNormalizeError;

/// Canonical types — the stable representation shared across all providers.
pub use types::{ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolResult};

/// The provider enum — identifies which wire format to use.
pub use provider::Provider;

/// The three public entry-point functions.
pub use normalize::{denormalize_definition, denormalize_result, normalize};

/// Convenience type alias: `Result<T>` expands to `Result<T, ToolNormalizeError>`.
///
/// Allows crate consumers to write `use operon_context_normalize_tools::Result;`
/// and then use `Result<ToolCall>` rather than the fully qualified form.
pub type Result<T> = std::result::Result<T, ToolNormalizeError>;
