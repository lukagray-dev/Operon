//! Argument types for the `ask` tool.
//!
//! Defines the parsing logic for the model's input to the `ask` tool
//! in the new body-based format.
//!
//! NEW CALL FORMAT:
//!   <ask>
//!   <<<<
//!   question="question here"
//!   option1="first option content"
//!   option2="second option content"
//!   option3="third option content"
//!   >>>>
//!
//! The body arrives as `args_json["__body__"]` (a String).
//! No path attr — ask is a global tool with no directory scope.

// ─────────────────────────────────────────────────────────────────────────────
// AskArgs
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments the model passes when calling the `ask` tool, parsed from the body.
///
/// The UI always adds a 4th free-text field automatically — the model only
/// supplies 3 pre-defined options as separate body keys.
#[derive(Debug)]
pub struct AskArgs {
    /// The question to present to the user.
    pub question: String,

    /// First pre-defined answer option.
    pub option1: String,

    /// Second pre-defined answer option.
    pub option2: String,

    /// Third pre-defined answer option.
    /// The UI adds a 4th free-text field for custom user input automatically.
    pub option3: String,
}

impl AskArgs {
    /// Parse `AskArgs` from the raw JSON arguments of a tool call.
    ///
    /// `question`, `option1`, `option2`, and `option3` are all parsed from XML attributes.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` with a descriptive message if any of the required attributes are missing.
    pub fn parse(args_json: &serde_json::Value) -> Result<AskArgs, String> {
        let question = args_json
            .get("question")
            .ok_or_else(|| "missing attribute: question".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'question' must be a string".to_string())?
            .trim()
            .to_string();

        let option1 = args_json
            .get("option1")
            .ok_or_else(|| "missing attribute: option1".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'option1' must be a string".to_string())?
            .trim()
            .to_string();

        let option2 = args_json
            .get("option2")
            .ok_or_else(|| "missing attribute: option2".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'option2' must be a string".to_string())?
            .trim()
            .to_string();

        let option3 = args_json
            .get("option3")
            .ok_or_else(|| "missing attribute: option3".to_string())?
            .as_str()
            .ok_or_else(|| "attribute 'option3' must be a string".to_string())?
            .trim()
            .to_string();

        Ok(AskArgs {
            question,
            option1,
            option2,
            option3,
        })
    }
}
