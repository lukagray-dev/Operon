//! Tests for the `ask` tool argument parsing and output serialization.

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::args::AskArgs;
    use crate::output::AskOutput;

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
        assert_eq!(parsed.options, ["JSON", "TOML", "YAML"]);
    }

    #[test]
    fn test_missing_question_fails() {
        let args = json!({
            "options": ["A", "B", "C"]
        });
        let result = AskArgs::from_json(&args);
        assert!(result.is_err(), "missing question should fail to parse");
    }

    #[test]
    fn test_missing_options_fails() {
        let args = json!({
            "question": "Choose one:"
        });
        let result = AskArgs::from_json(&args);
        assert!(result.is_err(), "missing options should fail to parse");
    }

    #[test]
    fn test_two_options_fails() {
        // Array deserialization into [String; 3] requires exactly 3 elements.
        let args = json!({
            "question": "Pick one:",
            "options": ["A", "B"]
        });
        let result = AskArgs::from_json(&args);
        assert!(
            result.is_err(),
            "2 options should fail — exactly 3 required"
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

    // ── AskOutput serialization ────────────────────────────────────────────────

    #[test]
    fn test_output_serializes_correctly() {
        let output = AskOutput {
            answer: "JSON".to_string(),
        };
        let serialized = serde_json::to_value(&output).unwrap();
        assert_eq!(serialized, json!({ "answer": "JSON" }));
    }

    #[test]
    fn test_output_roundtrip() {
        let original = AskOutput {
            answer: "custom user input".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: AskOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.answer, original.answer);
    }
}
