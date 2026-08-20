//! Google Gemini reasoning and thinking budget capability detection.
//!
//! Hey friend! Google Gemini supports thinking budgets across Gemini 2.0 Flash Thinking,
//! Gemini 2.5 (Pro and Flash), and Gemini 3.x series models.
//!
//! We explicitly exclude media preview models (like `-tts`, `-image`, or embedding models)
//! that do not have text generation thinking capabilities.

/// Detects reasoning levels for Google Gemini models.
pub fn detect_gemini_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    let is_media_preview = id_lower.contains("-tts")
        || id_lower.contains("-image")
        || id_lower.contains("embedding")
        || id_lower.contains("aqa");

    if !is_media_preview
        && (id_lower.contains("thinking")
            || id_lower.contains("2.5")
            || id_lower.contains("3.7")
            || id_lower.contains("3.1")
            || id_lower.contains("3-flash")
            || id_lower.contains("3-pro")
            || id_lower.contains("2.0-flash"))
    {
        vec![
            "Low".to_string(),
            "Medium".to_string(),
            "High".to_string(),
            "Disabled".to_string(),
        ]
    } else {
        // Standard legacy models, Gemma models without thinking, and media variants
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_37_flash_thinking() {
        let levels = detect_gemini_reasoning("gemini-3.7-flash");
        assert_eq!(levels, vec!["Low", "Medium", "High", "Disabled"]);
    }

    #[test]
    fn test_gemini_25_pro_thinking() {
        let levels = detect_gemini_reasoning("gemini-2.5-pro");
        assert_eq!(levels, vec!["Low", "Medium", "High", "Disabled"]);
    }

    #[test]
    fn test_gemini_tts_preview_no_thinking() {
        let levels = detect_gemini_reasoning("gemini-2.5-flash-preview-tts");
        assert!(levels.is_empty());
    }
}
