//! # operon-context-normalize-messages
//!
//! Canonical conversation-message types and bidirectional wire-format conversion
//! for eleven major LLM providers.
//!
//! ## What this crate does
//!
//! Exactly one job:
//!
//! ```text
//! provider message JSON  ->  canonical internal types  ->  provider message JSON
//! ```
//!
//! No HTTP, no execution, no persistence, no async, no I/O. Pure message-shape
//! normalization and denormalization.
//!
//! ## Supported providers
//!
//! | Provider | Wire format family |
//! |---|---|
//! | [`Provider::Anthropic`] | Anthropic Messages API |
//! | [`Provider::OpenAI`] | OpenAI Chat Completions API |
//! | [`Provider::Gemini`] | Google Gemini GenerateContent API |
//! | [`Provider::Ollama`] | OpenAI-compatible + native `/api/chat` |
//! | [`Provider::DeepSeek`] | OpenAI-compatible with `reasoning_content` |
//! | [`Provider::OpenRouter`] | Auto-detects OpenAI or Anthropic shape |
//! | [`Provider::Groq`] | OpenAI-compatible |
//! | [`Provider::Mistral`] | OpenAI-compatible |
//! | [`Provider::XAI`] | OpenAI-compatible with `reasoning_content` |
//! | [`Provider::NvidiaNim`] | OpenAI-compatible with `reasoning_content` |
//! | [`Provider::Cohere`] | Cohere v2 Chat API |
//!
//! ## Quick start
//!
//! ```rust
//! use operon_context_normalize_messages::{
//!     normalize_message, denormalize_messages, ConversationMessage, ContentBlock,
//!     MessageRole, Provider,
//! };
//! use serde_json::json;
//!
//! let raw = json!({
//!     "role": "user",
//!     "content": "Hello"
//! });
//! let msg = normalize_message(raw, &Provider::OpenAI).unwrap();
//! assert_eq!(msg.role, MessageRole::User);
//! assert_eq!(msg.content, vec![ContentBlock::Text("Hello".to_string())]);
//!
//! let wire = denormalize_messages(&[ConversationMessage::system("You are helpful."), msg], &Provider::OpenAI).unwrap();
//! assert!(wire.get("messages").is_some());
//! ```

// Declare all modules.
pub mod error;
pub mod normalize;
pub mod provider;
pub mod stop_reason;
pub mod types;

/// The single error type for all message normalization/denormalization operations.
pub use error::MessageNormalizeError;

/// Convenience result alias used by this crate.
pub use error::Result;

/// Public entry points.
pub use normalize::{denormalize_messages, normalize_message};

/// Provider enum used to select wire-format behavior.
pub use provider::Provider;

/// Canonical message types.
pub use types::{
    ContentBlock, ConversationMessage, DocumentBlock, DocumentSource, ImageBlock, ImageSource,
    MessageRole, StopReason,
};
