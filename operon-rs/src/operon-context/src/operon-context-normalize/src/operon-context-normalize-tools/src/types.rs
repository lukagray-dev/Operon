//! Canonical tool-call types used throughout `operon-context-normalize-tools`.
//!
//! These types are the **single source of truth** that sits between LLM provider
//! wire formats and your application code. Every provider-specific wire format is
//! normalized *into* these types on the way in ([`normalize`](crate::normalize)),
//! and serialized *from* these types on the way out
//! ([`denormalize_definition`](crate::denormalize_definition),
//! [`denormalize_result`](crate::denormalize_result)).
//!
//! # Workflow
//! ```text
//! Provider wire JSON
//!   → normalize()        → ToolCall        (model wants to call a tool)
//!   → [your code runs the tool]
//!   → ToolResult
//!   → denormalize_result() → provider wire JSON   (feed result back into context)
//!
//! ToolDefinition
//!   → denormalize_definition() → provider wire JSON (tell the model what tools exist)
//! ```

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// ToolCallId
// ─────────────────────────────────────────────────────────────────────────────

/// A strongly-typed wrapper around a tool-call identifier string.
///
/// Different providers use different ID schemes — Anthropic uses `"toolu_01A"`,
/// OpenAI uses `"call_abc123"`, Gemini generates IDs synthetically because its
/// wire format provides none. Wrapping the raw string in a newtype makes
/// ID-confusion bugs a compile-time error instead of a silent runtime mismatch.
///
/// # Example
/// ```
/// # use operon_context_normalize_tools::ToolCallId;
/// let id = ToolCallId("call_abc123".to_string());
/// println!("{}", id.0); // "call_abc123"
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallId(pub String);

// ─────────────────────────────────────────────────────────────────────────────
// ToolCall
// ─────────────────────────────────────────────────────────────────────────────

/// A single tool-call emitted by the model during a generation.
///
/// When a model decides to use a tool, it produces one or more `ToolCall` values
/// in its response. Your code is responsible for dispatching each call to the
/// actual tool implementation and feeding the result back via [`ToolResult`].
///
/// The `arguments` field is **always** a parsed JSON object (`serde_json::Value::Object`),
/// even for providers (like OpenAI) that transmit arguments as a JSON-encoded string.
/// The decoding is handled transparently by [`normalize`](crate::normalize).
///
/// # Example
/// ```
/// # use operon_context_normalize_tools::{normalize, Provider};
/// # use serde_json::json;
/// let wire = json!({
///     "id": "call_abc",
///     "type": "function",
///     "function": { "name": "read_file", "arguments": "{\"path\":\"/foo\"}" }
/// });
/// let call = normalize(wire, &Provider::OpenAI).unwrap();
/// assert_eq!(call.name, "read_file");
/// assert_eq!(call.arguments["path"], "/foo");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique ID for this call. Used to pair the call with its [`ToolResult`].
    /// Must be echoed back in `ToolResult::call_id`.
    pub id: ToolCallId,

    /// The exact name of the tool the model wants to invoke, e.g. `"read_file"`.
    pub name: String,

    /// The arguments the model passed to the tool. Always a parsed JSON object —
    /// never a raw string, regardless of which provider sent the wire message.
    pub arguments: serde_json::Value,
}

// ─────────────────────────────────────────────────────────────────────────────
// ToolResult
// ─────────────────────────────────────────────────────────────────────────────

/// The outcome of executing a tool call, ready to be inserted back into context.
///
/// After your code runs the tool described by a [`ToolCall`], wrap the output in a
/// `ToolResult` and pass it to [`denormalize_result`](crate::denormalize_result)
/// to get the provider-specific message format that can be appended to the
/// conversation history.
///
/// # Example
/// ```
/// # use operon_context_normalize_tools::{
/// #     denormalize_result, Provider, ToolCallId, ToolContent, ToolResult
/// # };
/// let result = ToolResult {
///     call_id: ToolCallId("call_abc".to_string()),
///     name: "read_file".to_string(),
///     content: ToolContent::Text("contents of /foo".to_string()),
///     is_error: false,
/// };
/// let wire = denormalize_result(&result, &Provider::OpenAI).unwrap();
/// assert_eq!(wire["role"], "tool");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// The `id` of the [`ToolCall`] this result is answering.
    /// Must match exactly — providers use this to correlate calls with results.
    pub call_id: ToolCallId,

    /// The name of the tool that produced this result. Some providers (e.g. OpenAI)
    /// require the name to appear in the result message in addition to the call ID.
    pub name: String,

    /// The content returned by the tool execution.
    pub content: ToolContent,

    /// Whether the tool execution produced an error. When `true`, some providers
    /// (e.g. Anthropic) treat the result message differently, allowing the model
    /// to recover or retry.
    pub is_error: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// ToolContent
// ─────────────────────────────────────────────────────────────────────────────

/// The content payload carried by a [`ToolResult`].
///
/// Tools may return either plain text or structured JSON data. Use [`ToolContent::Text`]
/// for human-readable strings (file contents, command output, error messages), and
/// [`ToolContent::Json`] for structured data the model is expected to reason about
/// as an object (API responses, database rows, etc.).
///
/// Both variants are losslessly representable in all supported provider wire formats.
/// For providers that only accept string content in tool results (e.g. OpenAI's
/// `"content"` string field), `Json` values are serialized to a compact JSON string.
///
/// The `#[serde(untagged)]` attribute ensures canonical serialization uses the bare
/// inner value (a JSON string or a JSON object), not a tagged wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolContent {
    /// Plain UTF-8 text returned by the tool.
    Text(String),

    /// Structured JSON data returned by the tool.
    Json(serde_json::Value),
}

// ─────────────────────────────────────────────────────────────────────────────
// ToolDefinition
// ─────────────────────────────────────────────────────────────────────────────

/// A description of a tool that the model may call.
///
/// Sent to the provider as part of the request so the model knows which tools
/// are available and how to call them. The quality of `description` directly
/// affects whether the model invokes the tool correctly and in the right situations.
///
/// # Example
/// ```
/// # use operon_context_normalize_tools::{
/// #     denormalize_definition, Provider, ToolDefinition
/// # };
/// # use serde_json::json;
/// let def = ToolDefinition {
///     name: "read_file".to_string(),
///     description: "Read the contents of a file at the given path.".to_string(),
///     parameters: json!({
///         "type": "object",
///         "properties": {
///             "path": { "type": "string", "description": "Absolute path to the file" }
///         },
///         "required": ["path"]
///     }),
/// };
/// let wire = denormalize_definition(&def, &Provider::Anthropic).unwrap();
/// assert!(wire.get("input_schema").is_some()); // Anthropic uses "input_schema" not "parameters"
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The unique name for this tool. Must be unique within a single request.
    /// Use snake_case identifiers (e.g. `"read_file"`, `"search_web"`).
    pub name: String,

    /// A clear, concise description of what this tool does and when to use it.
    /// This is the primary signal the model uses to decide whether to call the tool.
    pub description: String,

    /// A JSON Schema `object` describing the tool's accepted arguments.
    ///
    /// Expected shape:
    /// ```json
    /// {
    ///   "type": "object",
    ///   "properties": {
    ///     "param_name": { "type": "string", "description": "..." }
    ///   },
    ///   "required": ["param_name"]
    /// }
    /// ```
    ///
    /// For Cohere, this JSON Schema is automatically converted to Cohere's
    /// `parameter_definitions` format by [`denormalize_definition`](crate::denormalize_definition).
    pub parameters: serde_json::Value,
}
