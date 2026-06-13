//! Tests for the web_search tool.
//!
//! Network tests are marked with #[ignore] by default so they don't run in CI.
//! Run them manually with: cargo test -p operon-tools-web-search -- --ignored

#[cfg(test)]
mod tests {
    use crate::execute;
    use operon_context_normalize::tools::{ToolCallId, ToolContent};

    use serde_json::json;

    // ============================================================================
    // Non-network tests (run by default)
    // ============================================================================

    #[tokio::test]
    async fn test_empty_query_error() {
        // Empty query should return an error.
        let result = execute(
            ToolCallId("call_1".to_string()),
            json!({
                "query": ""
            }),
        )
        .await
        .unwrap();

        assert!(result.is_error);
        match &result.content {
            ToolContent::Text(msg) => assert!(msg.contains("empty")),
            _ => panic!("expected Text error"),
        }
    }

    #[tokio::test]
    async fn test_whitespace_query_error() {
        // Whitespace-only query should return an error.
        let result = execute(
            ToolCallId("call_2".to_string()),
            json!({
                "query": "   "
            }),
        )
        .await
        .unwrap();

        assert!(result.is_error);
        match &result.content {
            ToolContent::Text(msg) => assert!(msg.contains("empty")),
            _ => panic!("expected Text error"),
        }
    }

    // ============================================================================
    // Network tests (marked #[ignore], run with --ignored flag)
    // ============================================================================

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_basic_search() {
        // Query "rust programming language" and verify basic structure.
        let result = execute(
            ToolCallId("call_3".to_string()),
            json!({
                "query": "rust programming language"
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                // Should list results starting with "1. "
                assert!(text.contains("1. "));
                assert!(!text.is_empty(), "expected some search results");
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_max_results_respected() {
        // Query with max: "3" and verify we get at most 3 results.
        let result = execute(
            ToolCallId("call_4".to_string()),
            json!({
                "query": "rust lang",
                "max": "3"
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                // Since results are 1. , 2. , 3. , we check if it doesn't contain "4. "
                assert!(!text.contains("4. "), "expected at most 3 results, but got 4 or more: {}", text);
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_max_results_cap_enforced() {
        // Request max: "999" and verify it's capped at 10.
        let result = execute(
            ToolCallId("call_5".to_string()),
            json!({
                "query": "rust",
                "max": "999"
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                // Capped at 10, so it should not contain "11. "
                assert!(!text.contains("11. "), "expected at most 10 results (cap)");
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_no_results() {
        // Query something that's unlikely to have results.
        let result = execute(
            ToolCallId("call_6".to_string()),
            json!({
                "query": "xyzabc123notarealquery"
            }),
        )
        .await
        .unwrap();

        // Even with no results, is_error should be false.
        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                assert!(text.contains("No results for"));
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_site_search() {
        // Test DuckDuckGo site: syntax.
        let result = execute(
            ToolCallId("call_7".to_string()),
            json!({
                "query": "site:github.com rust async"
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                // If we got results, they should contain github.com
                if !text.contains("No results for") {
                    assert!(text.contains("github.com"), "expected github.com in results, got: {}", text);
                }
            }
            _ => panic!("expected Text response"),
        }
    }
}
