//! Anthropic reasoning and extended thinking capability detection.
//!
//! Hey friend! Anthropic introduced extended thinking starting with Claude 3.7 Sonnet
//! and Claude 4. It supports thinking budgets corresponding to Low, Medium, High, and Max,
//! plus Disabled (to turn off extended thinking).

/// Detects reasoning levels for Anthropic models.
pub fn detect_anthropic_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    // Claude 3.7+ and Claude 4+ families support extended thinking up to Max
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
    } else {
        // Standard models like Claude 3.5 Sonnet, Claude 3 Haiku do not support reasoning effort
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_37_has_max_thinking() {
        let levels = detect_anthropic_reasoning("claude-3-7-sonnet-20250219");
        assert_eq!(levels, vec!["Low", "Medium", "High", "Max", "Disabled"]);
    }

    #[test]
    fn test_claude_35_has_no_thinking() {
        let levels = detect_anthropic_reasoning("claude-3-5-sonnet-20241022");
        assert!(levels.is_empty());
    }
}
