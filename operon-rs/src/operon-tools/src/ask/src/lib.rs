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
            description: "Pauses the agent loop and presents the user a multiple-choice question. \
                          Write question, option1, option2, option3 in the tool body. The UI adds a \
                          free-text field as a 4th option automatically. Execution resumes when the \
                          user responds."
                .to_string(),
        },
        detailed: ToolDefinition {
            name: "ask".to_string(),
            description: "Pauses the agent loop and presents the user a multiple-choice question.\n\
                          \n\
                          ## Call format\n\
                          \n\
                          <ask>\n\
                          <<<<\n\
                          question=\"Which approach should I take?\"\n\
                          option1=\"Use the existing module\"\n\
                          option2=\"Rewrite from scratch\"\n\
                          option3=\"Ask for more context first\"\n\
                          >>>>\n\
                          \n\
                          ## Body keys\n\
                          \n\
                          - `question` (required): The question text to display to the user.\n\
                          - `option1` (required): First pre-defined answer option.\n\
                          - `option2` (required): Second pre-defined answer option.\n\
                          - `option3` (required): Third pre-defined answer option.\n\
                          \n\
                          The UI automatically adds a 4th free-text field for custom answers.\n\
                          All four body keys are required — missing any causes a parse error.\n\
                          \n\
                          ## Behavior\n\
                          \n\
                          The agent loop suspends immediately when `ask` is called. No further tool \
                          calls or model turns run until the user selects an option or types a custom \
                          answer. The user's response is returned as a plain-text ToolResult.\n\
                          \n\
                          ## Response format\n\
                          \n\
                          Plain text, two lines:\n\
                          - Numbered option: `Question: {question}\\nUser chose: option N. {option_content}`\n\
                          - Free-text input: `Question: {question}\\nUser wrote: {custom_text}`\n\
                          \n\
                          ## Common mistakes\n\
                          \n\
                          - Missing any of `question`, `option1`, `option2`, `option3` → parse error.\n\
                          - Calling `ask` multiple times in one turn → only the first call \
                            suspends the loop; remaining calls execute in subsequent turns.\n\
                            - Phrasing options as questions instead of concise answers → confuses users.\n\
                            - Expecting a 4th option in the ToolResult → the free-text field is UI-only; \
                              the answer arrives as a plain string regardless of which field the user used."
                  .to_string(),
          },
      }
  }
