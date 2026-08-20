//! OpenRouter gateway reasoning capability detection.
//!
//! OpenRouter hosts hundreds of models across all providers. It supports models with
//! reasoning capabilities (such as Claude 3.7, o1/o3, DeepSeek R1, Gemini 2.5/3, QwQ).

/// Detects reasoning levels for models routed through OpenRouter.
pub fn detect_openrouter_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    if id_lower.contains("claude-3-7")
        || id_lower.contains("claude-3.7")
        || id_lower.contains("claude-4")
    {
        vec![
            "Low".to_string(),
            "Medium".to_string(),
            "High".to_string(),
            "Max".to_string(),
            "Disabled".to_string(),
        ]
    } else if id_lower.contains("r1")
        || id_lower.contains("qwq")
        || id_lower.starts_with("openai/o1")
        || id_lower.starts_with("openai/o3")
        || id_lower.contains("thinking")
        || id_lower.contains("reasoner")
        || id_lower.contains("gemini-2.5")
        || id_lower.contains("gemini-3")
    {
        vec![
            "Low".to_string(),
            "Medium".to_string(),
            "High".to_string(),
            "Disabled".to_string(),
        ]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_claude_37() {
        assert_eq!(
            detect_openrouter_reasoning("anthropic/claude-3.7-sonnet"),
            vec!["Low", "Medium", "High", "Max", "Disabled"]
        );
    }

    #[test]
    fn test_openrouter_r1() {
        assert_eq!(
            detect_openrouter_reasoning("deepseek/deepseek-r1"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
    }
}
