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
    /// `question`, `option1`, `option2`, and `option3` are all parsed from
    /// `args_json["__body__"]` as key=value lines (one per line).
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` with a descriptive message if:
    /// - `__body__` key is missing entirely.
    /// - `question` is missing from the body.
    /// - Any of `option1`, `option2`, or `option3` are missing from the body.
    pub fn parse(args_json: &serde_json::Value) -> Result<AskArgs, String> {
        // ── Extract and parse the body ────────────────────────────────────────
        // The body contains all arguments as key=value lines, already unquoted
        // by the call-format parser.
        let body = args_json["__body__"]
            .as_str()
            .ok_or_else(|| "missing body".to_string())?;

        let mut question: Option<String> = None;
        let mut option1: Option<String> = None;
        let mut option2: Option<String> = None;
        let mut option3: Option<String> = None;

        for line in body.lines() {
            let line = line.trim();
            // Skip blank lines gracefully — body may have padding.
            if line.is_empty() {
                continue;
            }

            // Each non-empty line must be in `key=value` form.
            // The value is the portion after the first `=`, already unquoted by the parser.
            if let Some(eq) = line.find('=') {
                let key = line[..eq].trim();
                let val = line[eq + 1..].trim().to_string();

                match key {
                    "question" => question = Some(val),
                    "option1" => option1 = Some(val),
                    "option2" => option2 = Some(val),
                    "option3" => option3 = Some(val),
                    // Unknown keys are silently ignored — forward compatibility.
                    _ => {}
                }
            }
        }

        // All four fields are required — missing any is always a model error.
        Ok(AskArgs {
            question: question.ok_or("missing body key: question")?,
            option1: option1.ok_or("missing body key: option1")?,
            option2: option2.ok_or("missing body key: option2")?,
            option3: option3.ok_or("missing body key: option3")?,
        })
    }
}
