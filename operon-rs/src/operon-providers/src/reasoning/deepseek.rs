//! DeepSeek reasoning capability detection.
//!
//! DeepSeek reasoning models like `deepseek-reasoner` and `deepseek-r1` emit explicit
//! thinking traces and support reasoning modes.

/// Detects reasoning levels for DeepSeek models.
pub fn detect_deepseek_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    if id_lower.contains("reasoner") || id_lower.contains("r1") {
        vec![
            "Low".to_string(),
            "Medium".to_string(),
            "High".to_string(),
            "Disabled".to_string(),
        ]
    } else {
        // Standard chat models like deepseek-chat
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deepseek_reasoner() {
        assert_eq!(
            detect_deepseek_reasoning("deepseek-reasoner"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
    }

    #[test]
    fn test_deepseek_chat() {
        assert!(detect_deepseek_reasoning("deepseek-chat").is_empty());
    }
}
