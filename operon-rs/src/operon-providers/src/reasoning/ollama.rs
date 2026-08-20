//! Ollama local model reasoning capability detection.
//!
//! Ollama hosts diverse open weights models. Reasoning-capable models include
//! DeepSeek-R1 distills, QwQ, and explicit thinking fine-tunes.

/// Detects reasoning levels for local models served by Ollama.
pub fn detect_ollama_reasoning(model_id: &str) -> Vec<String> {
    let id_lower = model_id.to_lowercase();

    if id_lower.contains("r1")
        || id_lower.contains("qwq")
        || id_lower.contains("thinking")
        || id_lower.contains("reasoning")
        || id_lower.contains("reasoner")
    {
        vec![
            "Low".to_string(),
            "Medium".to_string(),
            "High".to_string(),
            "Disabled".to_string(),
        ]
    } else {
        // Standard models like llama3, mistral, gemma
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_r1_qwq() {
        assert_eq!(
            detect_ollama_reasoning("deepseek-r1:14b"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
        assert_eq!(
            detect_ollama_reasoning("qwq:32b"),
            vec!["Low", "Medium", "High", "Disabled"]
        );
    }

    #[test]
    fn test_ollama_standard_llama() {
        assert!(detect_ollama_reasoning("llama3.2:3b").is_empty());
    }
}
