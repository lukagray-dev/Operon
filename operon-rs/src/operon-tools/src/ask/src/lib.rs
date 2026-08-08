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
use operon_tools_core::TieredToolDefinition;
use serde_json::json;

/// Returns the tiered tool definition for the `ask` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the key constraint (exactly 3 options).
/// - `detailed`: sent after a malformed call. Full explanation with argument
///   shapes, behavior, response format, and common mistakes.
pub fn definition() -> TieredToolDefinition {
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
                "description": "Exactly 3 answer options. The UI adds a free-text field as a 4th option."
            }
        }
    });

    TieredToolDefinition {
        short: ToolDefinition {
            name: "ask".to_string(),
            description: "Ask the user a multiple-choice question and wait for their answer. \
                          Provide exactly 3 options — the UI adds a free-text field as a 4th. \
                          The agent loop pauses until the user responds."
                .to_string(),
            parameters: parameters.clone(),
        },
        detailed: ToolDefinition {
            name: "ask".to_string(),
            description: "\
Asks the user a multiple-choice question and pauses the agent loop until they answer.

## Input shapes

`question` (required, string): Question to display to the user.
`options` (required, array of 3 strings): Exactly 3 answer choices (UI adds a 4th free-text field).

## Response format

Returns the user's selected option or custom text as plain text.

## Common mistakes & errors

- Passing fewer or more than 3 options. Returns error: \"expected exactly 3 options, got {n}. Provide exactly 3 pre-defined answer options — the UI adds a 4th free-text field automatically.\""
                .to_string(),
            parameters,
        },
    }
}
