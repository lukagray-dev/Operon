//! Tests for the web_fetch tool.
//!
//! Network tests are marked with #[ignore] by default so they don't run in CI.
//! Run them manually with: cargo test -p operon-tools-web-fetch -- --ignored

#[cfg(test)]
mod tests {
    use crate::execute;
    use operon_context_normalize::tools::{ToolCallId, ToolContent};

    use serde_json::json;

    // ============================================================================
    // Non-network tests (run by default)
    // ============================================================================

    #[tokio::test]
    async fn test_empty_url_error() {
        // Empty URL should return an error.
        let result = execute(
            ToolCallId("call_1".to_string()),
            json!({
                "url": ""
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
    async fn test_invalid_scheme_error() {
        // FTP URL should return an error (only http/https allowed).
        let result = execute(
            ToolCallId("call_2".to_string()),
            json!({
                "url": "ftp://example.com"
            }),
        )
        .await
        .unwrap();

        assert!(result.is_error);
        match &result.content {
            ToolContent::Text(msg) => assert!(msg.contains("http")),
            _ => panic!("expected Text error"),
        }
    }

    #[test]
    fn test_title_extraction() {
        // Unit test for title extraction helper.
        let html = "<html><head><title>Hello World</title></head></html>";
        let title = crate::executor::extract_title(html);
        assert_eq!(title, Some("Hello World".to_string()));
    }

    #[test]
    fn test_title_extraction_missing() {
        // Unit test for title extraction when title is missing.
        let html = "<html><body>no title</body></html>";
        let title = crate::executor::extract_title(html);
        assert_eq!(title, None);
    }

    #[test]
    fn test_title_extraction_empty() {
        // Unit test for title extraction when title is empty.
        let html = "<html><head><title></title></head></html>";
        let title = crate::executor::extract_title(html);
        assert_eq!(title, None);
    }

    #[test]
    fn test_plain_text_fallback() {
        // Unit test for plain text fallback HTML stripping.
        let html = "<p>Hello <b>world</b></p>";
        let text = crate::executor::plain_text_fallback(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("<p>"));
        assert!(!text.contains("<b>"));
    }

    #[test]
    fn test_plain_text_fallback_nested() {
        // Unit test for plain text fallback with nested tags.
        let html = "<div><p>Nested <span>content</span> here</p></div>";
        let text = crate::executor::plain_text_fallback(html);
        assert!(text.contains("Nested"));
        assert!(text.contains("content"));
        assert!(text.contains("here"));
    }

    // ============================================================================
    // Network tests (marked #[ignore], run with --ignored flag)
    // ============================================================================

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_fetch_known_url() {
        // Fetch a known URL and verify basic structure.
        let result = execute(
            ToolCallId("call_3".to_string()),
            json!({
                "url": "https://www.rust-lang.org"
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                assert!(text.contains("status: 200"));
                assert!(text.contains("title:"));
                assert!(!text.is_empty(), "content should not be empty");
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_fetch_404() {
        // Fetch a 404 URL and verify it's not an error (just a status code).
        let result = execute(
            ToolCallId("call_4".to_string()),
            json!({
                "url": "https://httpstat.us/404"
            }),
        )
        .await
        .unwrap();

        // 404 is NOT a tool error — the model receives the status code.
        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                assert!(text.contains("status: 404"));
                assert!(text.contains("no content — non-success status"));
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_truncation_flag() {
        // Verify that truncation notice is set correctly.
        // This test fetches a known large page and checks if truncation is reported.
        let result = execute(
            ToolCallId("call_6".to_string()),
            json!({
                "url": "https://en.wikipedia.org/wiki/Rust_(programming_language)"
            }),
        )
        .await
        .unwrap();

        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                if text.contains("[truncated") {
                    assert!(text.contains("showing first 10000]"));
                }
            }
            _ => panic!("expected Text response"),
        }
    }

    #[tokio::test]
    #[ignore = "requires network"]
    async fn test_redirect_followed() {
        // Test that HTTP redirects are followed.
        // Use a URL that redirects (e.g., http → https).
        let result = execute(
            ToolCallId("call_8".to_string()),
            json!({
                "url": "http://www.rust-lang.org"  // May redirect to https
            }),
        )
        .await
        .unwrap();

        // Should succeed (either directly or after redirect).
        assert!(!result.is_error);
        match &result.content {
            ToolContent::Text(text) => {
                // Status should be 200 (after redirect).
                assert!(text.contains("status: 200"));
            }
            _ => panic!("expected Text response"),
        }
    }
}
