//! Tests for the `ask` tool argument parsing and option count validation.

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::args::AskArgs;
    use crate::error::AskToolError;

    // ── AskArgs parsing ────────────────────────────────────────────────────────

    #[test]
    fn test_valid_args_parse() {
        let args = json!({
            "question": "Which format do you prefer?",
            "options": ["JSON", "TOML", "YAML"]
        });
        let parsed = AskArgs::from_json(&args);
        assert!(parsed.is_ok(), "valid args should parse successfully");
        let parsed = parsed.unwrap();
        assert_eq!(parsed.question, "Which format do you prefer?");
        assert_eq!(parsed.options, vec!["JSON", "TOML", "YAML"]);
    }

    #[test]
    fn test_missing_question_fails() {
        let args = json!({
            "options": ["A", "B", "C"]
        });
        let result = AskArgs::from_json(&args);
        assert!(result.is_err(), "missing question should fail to parse");
        match result.unwrap_err() {
            AskToolError::ArgsParse(_) => {}
            err => panic!("expected ArgsParse error, got {err:?}"),
        }
    }

    #[test]
    fn test_missing_options_fails() {
        let args = json!({
            "question": "Choose one:"
        });
        let result = AskArgs::from_json(&args);
        assert!(result.is_err(), "missing options should fail to parse");
        match result.unwrap_err() {
            AskToolError::ArgsParse(_) => {}
            err => panic!("expected ArgsParse error, got {err:?}"),
        }
    }

    #[test]
    fn test_two_options_fails() {
        let args = json!({
            "question": "Pick one:",
            "options": ["A", "B"]
        });
        let result = AskArgs::from_json(&args);
        assert!(
            result.is_err(),
            "2 options should fail — exactly 3 required"
        );
        let err = result.unwrap_err();
        match &err {
            AskToolError::WrongOptionCount(2) => {}
            _ => panic!("expected WrongOptionCount(2), got {err:?}"),
        }
        assert_eq!(
            err.to_string(),
            "expected exactly 3 options, got 2. Provide exactly 3 pre-defined answer options — the UI adds a 4th free-text field automatically."
        );
    }

    #[test]
    fn test_four_options_fails() {
        let args = json!({
            "question": "Pick one:",
            "options": ["A", "B", "C", "D"]
        });
        let result = AskArgs::from_json(&args);
        assert!(
            result.is_err(),
            "4 options should fail — exactly 3 required"
        );
        let err = result.unwrap_err();
        match &err {
            AskToolError::WrongOptionCount(4) => {}
            _ => panic!("expected WrongOptionCount(4), got {err:?}"),
        }
        assert_eq!(
            err.to_string(),
            "expected exactly 3 options, got 4. Provide exactly 3 pre-defined answer options — the UI adds a 4th free-text field automatically."
        );
    }

    #[test]
    fn test_empty_question_is_valid() {
        // Empty string is structurally valid — semantic validation is the model's job.
        let args = json!({
            "question": "",
            "options": ["Yes", "No", "Maybe"]
        });
        let result = AskArgs::from_json(&args);
        assert!(
            result.is_ok(),
            "empty question string is structurally valid"
        );
    }

    #[test]
    fn test_ask_defensive_aliases_and_stringified_options() {
        let args = json!({
            "prompt": "Pick your language",
            "choices": "[\"Rust\", \"Python\", \"TypeScript\"]"
        });
        let result = AskArgs::from_json(&args);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.question, "Pick your language");
        assert_eq!(parsed.options, vec!["Rust", "Python", "TypeScript"]);
    }
}
