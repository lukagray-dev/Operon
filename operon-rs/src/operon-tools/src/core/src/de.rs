//! # de — Shared defensive deserialization utilities for Operon tools.
//!
//! Hey friend! This module provides reusable Serde helper functions that make tool argument
//! deserialization resilient against common LLM formatting quirks across open-weights
//! (Nemotron, Llama, Qwen, DeepSeek, Mistral) and proprietary models.
//!
//! Handled quirks include:
//! 1. Stringified JSON arrays (e.g. `"[ \"a\", \"b\" ]"`)
//! 2. Markdown-fenced stringified arguments (e.g. ````"```json\n[...]\n```"````)
//! 3. Single items passed where an array was requested (`"group": "fs"` vs `["fs"]`)
//! 4. Numbers passed as strings (`"timeout": "5000"` vs `5000`)
//! 5. Numeric IDs passed as integers (`"id": 1` vs `"1"`)
//! 6. Empty path strings normalized to current directory (`""` -> `"."`)

use serde::{Deserialize, Deserializer};

/// Strips markdown code block fences (e.g. ```` ```json ... ``` ```` or ```` ``` ... ``` ````)
/// from a string if present.
pub fn strip_markdown_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    } else if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim();
        }
    }
    trimmed
}

/// Deserializes a `Vec<String>` from flexible shapes:
/// - Real JSON array: `["a", "b"]`
/// - Stringified JSON array: `"[ \"a\", \"b\" ]"`
/// - Markdown-fenced array: ````"```json\n[\"a\"]\n```"````
/// - Single string: `"a"` -> `vec!["a"]`
/// - Null or missing: `vec![]`
pub fn deserialize_flexible_string_list<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    parse_string_list_from_value(&value).map_err(serde::de::Error::custom)
}

/// Optional variant of `deserialize_flexible_string_list`.
pub fn deserialize_flexible_string_list_opt<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.is_null() {
        return Ok(None);
    }
    let list = parse_string_list_from_value(&value).map_err(serde::de::Error::custom)?;
    if list.is_empty() {
        Ok(None)
    } else {
        Ok(Some(list))
    }
}

/// Internal helper to parse string list from `serde_json::Value`.
fn parse_string_list_from_value(value: &serde_json::Value) -> Result<Vec<String>, String> {
    match value {
        serde_json::Value::Array(arr) => {
            let mut result = Vec::with_capacity(arr.len());
            for item in arr {
                match item {
                    serde_json::Value::String(s) => result.push(s.clone()),
                    serde_json::Value::Number(n) => result.push(n.to_string()),
                    other => result.push(other.to_string()),
                }
            }
            Ok(result)
        }
        serde_json::Value::String(s) => {
            let cleaned = strip_markdown_fences(s);
            // Try parsing string as JSON array first
            if let Ok(serde_json::Value::Array(inner_arr)) = serde_json::from_str(cleaned) {
                let mut result = Vec::with_capacity(inner_arr.len());
                for item in inner_arr {
                    match item {
                        serde_json::Value::String(s) => result.push(s),
                        serde_json::Value::Number(n) => result.push(n.to_string()),
                        other => result.push(other.to_string()),
                    }
                }
                Ok(result)
            } else if cleaned.is_empty() {
                Ok(Vec::new())
            } else {
                // Otherwise treat as a single string item
                Ok(vec![cleaned.to_string()])
            }
        }
        serde_json::Value::Null => Ok(Vec::new()),
        other => Ok(vec![other.to_string()]),
    }
}

/// Deserializes a field that could be either a single string or an array of strings into an `Option<String>`.
/// If passed an array (e.g. `groups: ["fs"]`), takes the first item.
pub fn deserialize_flexible_single_string_opt<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => {
            let cleaned = strip_markdown_fences(&s);
            if cleaned.is_empty() {
                Ok(None)
            } else {
                Ok(Some(cleaned.to_string()))
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(first) = arr.first() {
                match first {
                    serde_json::Value::String(s) => Ok(Some(s.clone())),
                    serde_json::Value::Number(n) => Ok(Some(n.to_string())),
                    other => Ok(Some(other.to_string())),
                }
            } else {
                Ok(None)
            }
        }
        serde_json::Value::Number(n) => Ok(Some(n.to_string())),
        serde_json::Value::Null => Ok(None),
        other => Ok(Some(other.to_string())),
    }
}

/// Deserializes an ID string from either a string `"1"` or an integer `1`.
pub fn deserialize_flexible_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected a string or integer ID, got {other}"
        ))),
    }
}

/// Deserializes an optional ID string from either a string `"1"`, integer `1`, or null/omitted.
pub fn deserialize_flexible_id_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        serde_json::Value::Number(n) => Ok(Some(n.to_string())),
        serde_json::Value::Null => Ok(None),
        other => Err(serde::de::Error::custom(format!(
            "expected a string or integer ID, got {other}"
        ))),
    }
}

/// Deserializes an optional `u64` from either a number `5000` or numeric string `"5000"`.
pub fn deserialize_flexible_u64_opt<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => n.as_u64().map(Some).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid positive integer: {n}"))
        }),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<u64>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
        serde_json::Value::Null => Ok(None),
        other => Err(serde::de::Error::custom(format!(
            "expected number or numeric string, got {other}"
        ))),
    }
}

/// Deserializes an optional `usize` from either a number `5` or numeric string `"5"`.
pub fn deserialize_flexible_usize_opt<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => n.as_u64().map(|v| Some(v as usize)).ok_or_else(|| {
            serde::de::Error::custom(format!("invalid positive integer: {n}"))
        }),
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                trimmed
                    .parse::<usize>()
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
        serde_json::Value::Null => Ok(None),
        other => Err(serde::de::Error::custom(format!(
            "expected number or numeric string, got {other}"
        ))),
    }
}

/// Deserializes a directory path, converting empty string `""` or null to `"."`.
pub fn deserialize_default_dir_path<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                Ok(".".to_string())
            } else {
                Ok(trimmed.to_string())
            }
        }
        serde_json::Value::Null => Ok(".".to_string()),
        other => Ok(other.to_string()),
    }
}

/// Default function returning `"."` for path fields when omitted.
pub fn default_dot_path() -> String {
    ".".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct TestList {
        #[serde(deserialize_with = "deserialize_flexible_string_list")]
        items: Vec<String>,
    }

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct TestU64 {
        #[serde(default, deserialize_with = "deserialize_flexible_u64_opt")]
        val: Option<u64>,
    }

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct TestId {
        #[serde(deserialize_with = "deserialize_flexible_id")]
        id: String,
    }

    #[derive(Debug, PartialEq, Eq, Deserialize)]
    struct TestDirPath {
        #[serde(default = "default_dot_path", deserialize_with = "deserialize_default_dir_path")]
        path: String,
    }

    #[test]
    fn test_flexible_string_list() {
        // Native array
        let t1: TestList = serde_json::from_value(json!({ "items": ["a", "b"] })).unwrap();
        assert_eq!(t1.items, vec!["a", "b"]);

        // Stringified JSON array
        let t2: TestList = serde_json::from_value(json!({ "items": "[\"a\", \"b\"]" })).unwrap();
        assert_eq!(t2.items, vec!["a", "b"]);

        // Markdown fenced JSON array
        let t3: TestList = serde_json::from_value(json!({ "items": "```json\n[\"a\", \"b\"]\n```" })).unwrap();
        assert_eq!(t3.items, vec!["a", "b"]);

        // Single string item
        let t4: TestList = serde_json::from_value(json!({ "items": "single_item" })).unwrap();
        assert_eq!(t4.items, vec!["single_item"]);
    }

    #[test]
    fn test_flexible_u64() {
        let t1: TestU64 = serde_json::from_value(json!({ "val": 5000 })).unwrap();
        assert_eq!(t1.val, Some(5000));

        let t2: TestU64 = serde_json::from_value(json!({ "val": "5000" })).unwrap();
        assert_eq!(t2.val, Some(5000));

        let t3: TestU64 = serde_json::from_value(json!({})).unwrap();
        assert_eq!(t3.val, None);
    }

    #[test]
    fn test_flexible_id() {
        let t1: TestId = serde_json::from_value(json!({ "id": "123" })).unwrap();
        assert_eq!(t1.id, "123");

        let t2: TestId = serde_json::from_value(json!({ "id": 123 })).unwrap();
        assert_eq!(t2.id, "123");
    }

    #[test]
    fn test_default_dir_path() {
        let t1: TestDirPath = serde_json::from_value(json!({ "path": "src" })).unwrap();
        assert_eq!(t1.path, "src");

        let t2: TestDirPath = serde_json::from_value(json!({ "path": "" })).unwrap();
        assert_eq!(t2.path, ".");

        let t3: TestDirPath = serde_json::from_value(json!({})).unwrap();
        assert_eq!(t3.path, ".");
    }
}
