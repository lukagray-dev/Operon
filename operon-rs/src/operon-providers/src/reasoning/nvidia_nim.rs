//! NVIDIA NIM reasoning capability detection.

/// Detects reasoning levels for models hosted on NVIDIA NIM.
pub fn detect_nvidia_nim_reasoning(model_id: &str) -> Vec<String> {
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
