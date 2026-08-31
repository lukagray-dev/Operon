//! # operon-tools-ask
//!
//! Provides the `ask` tool definition and argument types.
//!
//! The `ask` tool lets the model pause the agent loop and present the user a
//! multiple-choice question with 3 pre-defined options and one free-text option.
//!
//! ## Execution model
//!
//! Unlike other tools, `ask` is NOT dispatched through `Dispatcher::dispatch()`.
//! The session runner intercepts `ask` calls before they reach the dispatcher,
//! emits `SessionEvent::AskQuestion`, and blocks on the command channel until
//! `SessionCommand::AskResponse` arrives. This mirrors how `ApprovalRequired`
//! works for policy Ask-mode decisions.
//!
//! This crate provides only:
//! - [`AskArgs`] — argument deserialization from the model's tool call JSON.
//! - [`AskToolError`] — parse error type.
//! - [`definition()`] — the `TieredToolDefinition` registered with the dispatcher.
//!
//! ## Tool schema
//!
//! ```json
//! {
//!   "name": "ask",
//!   "input_schema": {
//!     "type": "object",
//!     "required": ["question", "options"],
//!     "properties": {
//!       "question": { "type": "string" },
//!       "options": {
//!         "type": "array",
//!         "items": { "type": "string" },
//!         "minItems": 3,
//!         "maxItems": 3
//!       }
//!     }
//!   }
//! }
//! ```

mod args;
mod error;

#[cfg(test)]
mod tests;

pub use args::AskArgs;
pub use error::AskToolError;

use operon_context_normalize_tools::ToolDefinition;
use serde_json::json;

/// Returns the canonical tool definition for the `ask` tool.
///
/// Follows industry standards (OpenAI/Anthropic/Google function-calling specifications):
/// - Explicit required fields (`question`, `options`).
/// - Clear parameter description for question text and the 3 options.
pub fn definition() -> ToolDefinition {
    // Hey friend! We define the parameters schema for the interactive ask tool here.
    let parameters = json!({
        "type": "object",
        "required": ["question", "options"],
        "properties": {
            "question": {
                "type": "string",
                "description": "The question to ask the user."
            },
            "options": {
                "type": "array",
                "items": { "type": "string" },
                "minItems": 3,
                "maxItems": 3,
                "description": "Exactly 3 answer options. The UI automatically adds a free-text write-in field as a 4th option."
            }
        }
    });

    ToolDefinition {
        name: "ask".to_string(),
        description: "Ask the user a multiple-choice question and wait for their answer. \
                      Provide exactly 3 options — the UI adds a free-text field as a 4th. \
                      The agent loop pauses until the user responds."
            .to_string(),
        parameters,
    }
}
