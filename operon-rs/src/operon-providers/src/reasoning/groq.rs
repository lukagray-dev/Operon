//! Groq reasoning capability detection.

/// Detects reasoning levels for models hosted on Groq (e.g. DeepSeek R1 distills, QwQ).
pub fn detect_groq_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    if id_lower.contains("r1")
        || id_lower.contains("qwq")
        || id_lower.contains("thinking")
        || id_lower.contains("reasoner")
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
    fn test_groq_deepseek_r1_distill() {
        assert_eq!(
            detect_groq_reasoning("deepseek-r1-distill-llama-70b"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
    }
}
