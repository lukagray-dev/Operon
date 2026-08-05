//! # operon-tools-ask
//!
//! Provides the `ask` tool definition and argument/output types.
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
//! - [`AskOutput`] — the structured response shape written into the ToolResult.
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
mod output;

#[cfg(test)]
mod tests;

pub use args::AskArgs;
pub use error::AskToolError;
pub use output::AskOutput;

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
            description: "Ask the user a multiple-choice question and wait for their answer.\n\
                          \n\
                          ## Arguments\n\
                          \n\
                          - `question` (string, required): The question to display to the user.\n\
                          - `options` (array of exactly 3 strings, required): Pre-defined answer \
                            choices. The UI automatically adds a 4th free-text field for custom answers.\n\
                          \n\
                          ## Behavior\n\
                          \n\
                          The agent loop suspends immediately when `ask` is called. No further tool \
                          calls or model turns run until the user selects an option or types a custom \
                          answer. The user's response is returned as a structured ToolResult.\n\
                          \n\
                          ## Response shape\n\
                          \n\
                          `{ \"answer\": \"<user's answer>\" }` — either one of the 3 options verbatim, \
                          or the user's custom free-text input from the 4th field.\n\
                          \n\
                          ## Common mistakes\n\
                          \n\
                          - Passing fewer or more than 3 options → args parse error; the schema \
                            enforces exactly 3.\n\
                          - Calling `ask` multiple times in one turn → only the first call \
                            suspends the loop; remaining calls execute in subsequent turns.\n\
                          - Phrasing options as questions instead of concise answers → confuses users.\n\
                          - Expecting a 4th option in the ToolResult → the free-text field is UI-only; \
                            the answer arrives as a plain string regardless of which field the user used."
                .to_string(),
            parameters,
        },
    }
}
