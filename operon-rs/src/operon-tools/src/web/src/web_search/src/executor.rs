//! Executor for the web_search tool — handles DuckDuckGo search query logic.
//!
//! This module contains the core logic for executing search queries directly
//! using reqwest, parsing results from DuckDuckGo's static HTML page,
//! and formatting the plain-text output.

use crate::args::WebSearchArgs;
use operon_context_normalize::tools::{ToolCallId, ToolContent, ToolResult};

/// Default number of results to return if `max` is not specified.
const DEFAULT_RESULTS: usize = 5;

/// Maximum number of results to return, regardless of what the model requests.
const MAX_RESULTS: usize = 10;

struct RawSearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Executes the web_search tool with the given arguments.
///
/// Queries DuckDuckGo, parses the results, and returns them as plain text.
pub async fn execute(call_id: ToolCallId, args: WebSearchArgs) -> ToolResult {
    let max_results = args
        .max
        .unwrap_or(DEFAULT_RESULTS)
        .min(MAX_RESULTS)
        .max(1);

    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(15))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(format!("failed to initialize HTTP client: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    // Build search URL with query parameters
    let url = match reqwest::Url::parse_with_params(
        "https://html.duckduckgo.com/html/",
        &[("q", &args.query)],
    ) {
        Ok(u) => u,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(format!("failed to construct search URL: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    // Query DuckDuckGo's static HTML search page
    let response = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(format!("search failed: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    let html = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            return ToolResult {
                call_id,
                name: "web_search".to_string(),
                content: ToolContent::Text(format!("failed to read search response: {}", e)),
                is_error: true,
                read_paths: None,
            };
        }
    };

    let parsed_results = parse_duckduckgo_html(&html);

    if parsed_results.is_empty() {
        return ToolResult {
            call_id,
            name: "web_search".to_string(),
            content: ToolContent::Text(format!(
                "No results for '{}'. Try different search terms.",
                args.query
            )),
            is_error: false,
            read_paths: None,
        };
    }

    let text = parsed_results
        .into_iter()
        .take(max_results)
        .enumerate()
        .map(|(i, r)| {
            format!("{}. {}\n   {}\n   {}", i + 1, r.title, r.url, r.snippet)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    ToolResult {
        call_id,
        name: "web_search".to_string(),
        content: ToolContent::Text(text),
        is_error: false,
        read_paths: None,
    }
}

fn parse_duckduckgo_html(html: &str) -> Vec<RawSearchResult> {
    let mut results = Vec::new();
    
    // Split the HTML content into blocks corresponding to individual search results
    let parts: Vec<&str> = html.split("<div class=\"result").collect();
    if parts.len() <= 1 {
        return results;
    }
    
    // Skip the first part which is the page header
    for part in parts.into_iter().skip(1) {
        // Find the result__a class block
        let Some(a_start) = part.find("class=\"result__a\"") else { continue; };
        let a_block = &part[a_start..];
        
        let Some(href_start) = a_block.find("href=\"") else { continue; };
        let href_block = &a_block[href_start + 6..];
        let Some(href_end) = href_block.find("\"") else { continue; };
        let url = href_block[..href_end].to_string();
        
        let Some(title_start) = href_block[href_end..].find(">") else { continue; };
        let title_block = &href_block[href_end + title_start + 1..];
        let Some(title_end) = title_block.find("</a>") else { continue; };
        let title = clean_html_entities(&title_block[..title_end]);
        
        // Find the result__snippet class block
        let snippet = if let Some(snippet_start) = part.find("class=\"result__snippet\"") {
            let snippet_block = &part[snippet_start..];
            if let Some(text_start) = snippet_block.find(">") {
                let text_block = &snippet_block[text_start + 1..];
                if let Some(text_end) = text_block.find("</a>") {
                    clean_html_entities(&text_block[..text_end])
                } else if let Some(tag_end) = text_block.find("</") {
                    clean_html_entities(&text_block[..tag_end])
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };
        
        results.push(RawSearchResult { title, url, snippet });
    }
    
    results
}

fn clean_html_entities(html: &str) -> String {
    let mut s = String::new();
    let mut in_tag = false;
    
    // Strip nested HTML tags inside title or snippet
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => s.push(ch),
            _ => {}
        }
    }
    
    // Decode HTML entities commonly present in search snippets
    s.replace("&amp;", "&")
     .replace("&lt;", "<")
     .replace("&gt;", ">")
     .replace("&quot;", "\"")
     .replace("&#x27;", "'")
     .replace("&#39;", "'")
     .replace("&nbsp;", " ")
     .trim()
     .to_string()
}
