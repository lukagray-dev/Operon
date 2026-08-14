//! Tests for the web_search tool.
//!
//! Network tests are marked with #[ignore] by default so they don't run in CI.
//! Run them manually with: cargo test -p operon-tools-web-search -- --ignored

#[cfg(test)]
mod tests {
    use crate::{execute, output::SearchResult, WebSearchOutput};
    use operon_context_normalize_tools::{ToolCallId, ToolContent};
    use serde_json::json;

    // ============================================================================
    // Non-network tests (run by default)
    // ============================================================================

    #[test]
    fn test_to_plain_text_with_results() {
        let output = WebSearchOutput {
            query: "rust programming".to_string(),
            result_count: 2,
            results: vec![
                SearchResult {
                    rank: 1,
                    title: "Rust Language".to_string(),
                    url: "https://www.rust-lang.org".to_string(),
                    snippet: "Empowering everyone to build reliable and efficient software.".to_string(),
                },
                SearchResult {
                    rank: 2,
                    title: "Rust Github".to_string(),
                    url: "https://github.com/rust-lang/rust".to_string(),
                    snippet: "Empowering everyone to build reliable and efficient software.".to_string(),
                },
            ],
        };

        let text = output.to_plain_text();
        let expected = "Query: rust programming\n2 result(s)\n\n[1] Rust Language\n    https://www.rust-lang.org\n    Empowering everyone to build reliable and efficient software.\n\n[2] Rust Github\n    https://github.com/rust-lang/rust\n    Empowering everyone to build reliable and efficient software.";
        assert_eq!(text, expected);
    }

    #[test]
    fn test_to_plain_text_no_results() {
        let output = WebSearchOutput {
            query: "nonexistent query".to_string(),
            result_count: 0,
            results: vec![],
        };

        let text = output.to_plain_text();
        assert_eq!(text, "Query: nonexistent query\nNo results found.");
    }

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
                assert!(text.starts_with("Query: rust programming language"));
                assert!(text.contains("result(s)") || text.contains("No results found."));
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_max_results_respected() {
        // Query with max_results: 3 and verify we get output starting with query.
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
            ToolContent::Text(text) => {
                assert!(text.starts_with("Query: rust lang"));
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_max_results_cap_enforced() {
        // Request max_results: 999 and verify execution succeeds.
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
            ToolContent::Text(text) => {
                assert!(text.starts_with("Query: rust"));
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
                assert!(text.starts_with("Query: xyzabc123notarealquery"));
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
                assert!(text.starts_with("Query: site:github.com rust async"));
            }
            _ => panic!("expected Text response"),
        }
    }

    #[test]
    fn test_web_search_defensive_aliases() {
        use crate::WebSearchArgs;
        let args: WebSearchArgs = serde_json::from_value(json!({
            "q": "rust async",
            "limit": "3"
        }))
        .unwrap();

        assert_eq!(args.query, "rust async");
        assert_eq!(args.max_results, Some(3));
    }
}
