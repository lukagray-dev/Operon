//! Tests for the `ask` tool argument parsing.

#[cfg(test)]
mod tests {
    use crate::args::AskArgs;
    use serde_json::json;

    #[test]
    fn test_valid_args_parse() {
        let args = json!({
            "__body__": "question=\"Which format do you prefer?\"\noption1=\"JSON\"\noption2=\"TOML\"\noption3=\"YAML\""
        });
        let parsed = AskArgs::parse(&args);
        assert!(parsed.is_ok(), "valid args should parse successfully");
        let parsed = parsed.unwrap();
        assert_eq!(parsed.question, "Which format do you prefer?");
        assert_eq!(parsed.option1, "JSON");
        assert_eq!(parsed.option2, "TOML");
        assert_eq!(parsed.option3, "YAML");
    }

    #[test]
    fn test_missing_question_fails() {
        let args = json!({
            "__body__": "option1=\"JSON\"\noption2=\"TOML\"\noption3=\"YAML\""
        });
        let result = AskArgs::parse(&args);
        assert!(result.is_err(), "missing question should fail to parse");
    }

    #[test]
    fn test_missing_option_fails() {
        let args = json!({
            "__body__": "question=\"Choose one:\"\noption1=\"JSON\"\noption2=\"TOML\""
        });
        let result = AskArgs::parse(&args);
        assert!(result.is_err(), "missing option3 should fail to parse");
    }
}
