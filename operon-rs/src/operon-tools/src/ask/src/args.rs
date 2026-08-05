//! Argument types for the `ask` tool.
//!
//! Defines the deserialization schema for the model's input to the `ask` tool.
//! The tool accepts a question string and exactly 3 answer options.

use serde::Deserialize;

use crate::error::AskToolError;

/// Arguments the model passes when calling the `ask` tool.
///
/// The UI always adds a 4th free-text field automatically — the model only
/// supplies 3 pre-defined options.
#[derive(Debug, Deserialize)]
pub struct AskArgs {
    /// The question to present to the user.
    pub question: String,

    /// Exactly 3 pre-defined answer options.
    /// The UI adds a 4th free-text field for custom user input.
    pub options: [String; 3],
}

impl AskArgs {
    /// Deserialize from the raw JSON arguments of a tool call.
    ///
    /// Returns `Err(AskToolError::ArgsParse)` if the JSON shape is invalid —
    /// for example, missing `question`, missing `options`, or wrong array length.
    pub fn from_json(args: &serde_json::Value) -> Result<Self, AskToolError> {
        serde_json::from_value(args.clone()).map_err(AskToolError::ArgsParse)
    }
}
