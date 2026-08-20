//! xAI (Grok) reasoning capability detection.
//!
//! Grok 3 and reasoning-focused variants support configurable reasoning effort.

/// Detects reasoning levels for xAI models.
pub fn detect_xai_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    if id_lower.contains("grok-3")
        || id_lower.contains("reasoner")
        || id_lower.contains("grok-beta")
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
    fn test_xai_grok3() {
        assert_eq!(
            detect_xai_reasoning("grok-3-reasoner"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
    }

    #[test]
    fn test_xai_grok2() {
        assert!(detect_xai_reasoning("grok-2-1212").is_empty());
    }
}
