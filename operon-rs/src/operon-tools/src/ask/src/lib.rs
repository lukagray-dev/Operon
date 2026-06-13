//! # operon-tools-ask
//!
//! Provides the `ask` tool definition and argument types.
//!
//! The `ask` tool lets the model pause the agent loop and present the user a
//! multiple-choice question with 3 pre-defined options and one free-text option.
//!
//! ## Call format
//!
//! ```text
//! <ask>
//! <<<<
//! question="question here"
//! option1="first option content"
//! option2="second option content"
//! option3="third option content"
//! >>>>
//! ```
//!
//! No path attr — ask is a global tool with no directory scope.
//! All arguments arrive via `args_json["__body__"]`.
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
//! - [`AskArgs`] — argument parsing from the model's body-based tool call.
//! - [`AskToolError`] — parse error type.
//! - [`definition()`] — the `TieredToolDefinition` registered with the dispatcher.
//!
//! ## Response format (plain text)
//!
//! The session runner constructs the ToolResult as plain text:
//! - Numbered option choice: `"Question: {question}\nUser chose: option {N}. {option_content}"`
//! - Free-text (4th option): `"Question: {question}\nUser wrote: {custom_text}"`

mod args;
mod error;

#[cfg(test)]
mod tests;

pub use args::AskArgs;
pub use error::AskToolError;

use operon_context_normalize::tools::ToolDefinition;
use operon_tools_core::TieredToolDefinition;

/// Returns the tiered tool definition for the `ask` tool.
///
/// - `short`: sent to the model under normal conditions. Concise — states what
///   the tool does and the key constraint (3 options, body format).
/// - `detailed`: sent after a malformed call. Full explanation with call format,
///   behavior, response format, and common mistakes.
pub fn definition() -> TieredToolDefinition {
    TieredToolDefinition {
        short: ToolDefinition {
            name: "ask".to_string(),
            description: include_str!("description.md").to_string(),
        },
    }
}
