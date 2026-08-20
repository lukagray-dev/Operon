//! OpenAI reasoning effort capability detection.
//!
//! Hey friend! OpenAI models in the o-series (o1, o3, o4) and reasoning architectures
//! accept the `reasoning_effort` parameter with values Low, Medium, and High.

/// Detects reasoning levels for OpenAI models.
pub fn detect_openai_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    // Matches o1, o1-mini, o1-preview, o3-mini, o3, o4, gpt-5, and reasoning variants
    if id_lower.starts_with("o1")
        || id_lower.starts_with("o3")
        || id_lower.starts_with("o4")
        || id_lower.starts_with("gpt-5")
        || id_lower.contains("-reasoner")
    {
        vec![
            "Low".to_string(),
            "Medium".to_string(),
            "High".to_string(),
            "Disabled".to_string(),
        ]
    } else {
        // Standard GPT-4o, GPT-4o-mini, GPT-4-turbo, GPT-3.5-turbo models
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_o1_o3_reasoning() {
        assert_eq!(
            detect_openai_reasoning("o1"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
        assert_eq!(
            detect_openai_reasoning("o3-mini"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
    }

    #[test]
    fn test_gpt4o_no_reasoning() {
        assert!(detect_openai_reasoning("gpt-4o").is_empty());
        assert!(detect_openai_reasoning("gpt-4o-mini").is_empty());
    }
}
