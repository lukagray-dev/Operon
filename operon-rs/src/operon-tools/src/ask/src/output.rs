//! Output types for the `ask` tool.
//!
//! Defines the JSON shape returned to the model after the user responds.
//! The session runner constructs this and serializes it into the ToolResult.

use serde::{Deserialize, Serialize};

/// The structured response returned to the model after the user answers.
///
/// Serialized as `{ "answer": "<user's response>" }` in the ToolResult content.
/// The answer is either one of the 3 pre-defined options verbatim, or the user's
/// custom free-text input from the 4th field.
#[derive(Debug, Serialize, Deserialize)]
pub struct AskOutput {
    /// The user's chosen or typed answer.
    ///
    /// If the user selected one of the 3 pre-defined options, this is that
    /// option string verbatim. If the user typed a custom answer in the free-text
    /// field, this is that custom string.
    pub answer: String,
}
