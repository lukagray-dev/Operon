//! Cohere reasoning capability detection.

/// Detects reasoning levels for Cohere models.
pub fn detect_cohere_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    if id_lower.contains("reasoner") || id_lower.contains("thinking") {
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
