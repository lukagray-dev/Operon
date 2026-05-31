//! Tests for the web_search tool.
//!
//! Network tests are marked with #[ignore] by default so they don't run in CI.
//! Run them manually with: cargo test -p operon-tools-web-search -- --ignored

#[cfg(test)]
mod tests {
    use crate::{execute, WebSearchOutput};
    use operon_context_normalize_tools::{ToolCallId, ToolContent};
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

    #[tokio::test]
    async fn test_max_results_cap() {
        // Requesting max_results: 999 should be capped at 10.
        // This test doesn't make a real network call — it just verifies the cap logic.
        // We'll test this by checking that the executor respects the cap.
        // Since we can't easily mock the DuckDuckGo API, we'll skip this for now
        // and rely on the network test below.
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
            ToolContent::Json(json) => {
                let output: WebSearchOutput = serde_json::from_value(json.clone())
                    .expect("failed to deserialize WebSearchOutput");

                assert_eq!(output.query, "rust programming language");
                assert!(output.result_count > 0, "expected at least one result");
                assert_eq!(output.results[0].rank, 1, "first result should have rank 1");

                // Verify all results have non-empty title and url.
                for result in &output.results {
                    assert!(!result.title.is_empty(), "title should not be empty");
                    assert!(!result.url.is_empty(), "url should not be empty");
                }
            }
            _ => panic!("expected JSON response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_max_results_respected() {
        // Query with max_results: 3 and verify we get at most 3 results.
        let result = execute(
            ToolCallId("call_4".to_string()),
            json!({
                "query": "rust lang",
                "max_results": 3
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Json(json) => {
                let output: WebSearchOutput = serde_json::from_value(json.clone())
                    .expect("failed to deserialize WebSearchOutput");

                assert!(
                    output.results.len() <= 3,
                    "expected at most 3 results, got {}",
                    output.results.len()
                );
            }
            _ => panic!("expected JSON response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_max_results_cap_enforced() {
        // Request max_results: 999 and verify it's capped at 10.
        let result = execute(
            ToolCallId("call_5".to_string()),
            json!({
                "query": "rust",
                "max_results": 999
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Json(json) => {
                let output: WebSearchOutput = serde_json::from_value(json.clone())
                    .expect("failed to deserialize WebSearchOutput");

                assert!(
                    output.results.len() <= 10,
                    "expected at most 10 results (cap), got {}",
                    output.results.len()
                );
            }
            _ => panic!("expected JSON response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_no_results() {
        // Query something that's unlikely to have results.
        // This is a best-effort test — if DuckDuckGo finds results, the test will pass anyway.
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
            ToolContent::Json(json) => {
                let output: WebSearchOutput = serde_json::from_value(json.clone())
                    .expect("failed to deserialize WebSearchOutput");

                // result_count should match results.len()
                assert_eq!(output.result_count, output.results.len());
            }
            _ => panic!("expected JSON response"),
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
            ToolContent::Json(json) => {
                let output: WebSearchOutput = serde_json::from_value(json.clone())
                    .expect("failed to deserialize WebSearchOutput");

                // Should have results from github.com
                if output.result_count > 0 {
                    for result in &output.results {
                        assert!(
                            result.url.contains("github.com"),
                            "expected github.com in URL, got {}",
                            result.url
                        );
                    }
                }
            }
            _ => panic!("expected JSON response"),
        }
    }
}
