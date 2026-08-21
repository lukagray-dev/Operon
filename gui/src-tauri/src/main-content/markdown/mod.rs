//! Markdown Rendering Backend Module.
//!
//! This module provides high-performance Markdown-to-HTML compilation using `pulldown-cmark`.
//! It is designed for streaming LLM responses in Operon's GUI frontend.
//!
//! # Architecture & Pipeline:
//! 1. Frontend streams text tokens into TypeScript state.
//! 2. TypeScript invokes `render_markdown` (or batch) across the Tauri IPC boundary.
//! 3. `pulldown-cmark` parses the CommonMark/GFM stream and emits standardized HTML.
//! 4. Frontend injects the HTML into a `.markdown-body` container and enhances it with
//!    interactive features (e.g. code copy buttons, external link handling).

use pulldown_cmark::{html, Options, Parser};

/// Configures and returns standard GitHub-Flavored Markdown (GFM) parsing options.
///
/// Enables tables, task lists, strikethrough, footnotes, heading attributes,
/// and smart typography (quotes, em-dashes, ellipses).
pub fn gfm_options() -> Options {
    let mut options = Options::empty();
    // Enable GitHub Flavored Markdown Tables (| Header | Header |)
    options.insert(Options::ENABLE_TABLES);
    // Enable Task Lists ([ ] Todo, [x] Done)
    options.insert(Options::ENABLE_TASKLISTS);
    // Enable Strikethrough (~~deleted~~)
    options.insert(Options::ENABLE_STRIKETHROUGH);
    // Enable Footnotes ([^1])
    options.insert(Options::ENABLE_FOOTNOTES);
    // Enable Heading Attributes (# Title {#custom-id})
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    // Enable Smart Punctuation (converts straight quotes to curly quotes, --- to em-dash, ... to ellipsis)
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    // Enable LaTeX Math ($inline$ and $$display$$)
    options.insert(Options::ENABLE_MATH);
    options
}

/// Parses raw Markdown text and renders it into a standard HTML string.
///
/// # Examples:
/// ```
/// use gui_lib::main_content::markdown::parse_markdown_to_html;
///
/// let html = parse_markdown_to_html("# Hello Operon\nThis is **bold** text.");
/// assert!(html.contains("<h1>Hello Operon</h1>"));
/// assert!(html.contains("<strong>bold</strong>"));
/// ```
pub fn parse_markdown_to_html(markdown: &str) -> String {
    // If the input is empty or pure whitespace, return an empty string immediately.
    if markdown.trim().is_empty() {
        return String::new();
    }

    let options = gfm_options();
    let parser = Parser::new_ext(markdown, options);

    // Allocate an output buffer with a sensible capacity heuristic (1.3x input size)
    let mut html_output = String::with_capacity(markdown.len() + (markdown.len() / 3));
    html::push_html(&mut html_output, parser);

    html_output
}

/// Tauri IPC command: Compiles a single Markdown string into HTML.
///
/// Called by the frontend during streaming token updates and message loading.
#[tauri::command]
pub async fn render_markdown(markdown: String) -> Result<String, String> {
    Ok(parse_markdown_to_html(&markdown))
}

/// Tauri IPC command: Compiles multiple Markdown strings in a single batch.
///
/// Useful for initial session load when rendering multiple historical chat messages
/// without incurring round-trip IPC overhead per message.
#[tauri::command]
pub async fn render_markdown_batch(texts: Vec<String>) -> Result<Vec<String>, String> {
    let results = texts
        .into_iter()
        .map(|text| parse_markdown_to_html(&text))
        .collect();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_and_whitespace_markdown() {
        assert_eq!(parse_markdown_to_html(""), "");
        assert_eq!(parse_markdown_to_html("   \n\n\t  "), "");
    }

    #[test]
    fn test_headings_and_paragraphs() {
        let md = "# Heading 1\n\n## Heading 2\n\nParagraph with *italic* and **bold** text.";
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<h1>Heading 1</h1>"));
        assert!(html.contains("<h2>Heading 2</h2>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_gfm_tables() {
        let md = r#"
| Feature | Status | Speed |
| :--- | :---: | ---: |
| Tauri v2 | Ready | Ultra Fast |
| Pulldown | Active | Native |
"#;
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("Feature"));
        assert!(html.contains("Tauri v2"));
        assert!(html.contains("Ready"));
        assert!(html.contains("Ultra Fast"));
    }

    #[test]
    fn test_gfm_task_lists() {
        let md = r#"
- [ ] Incomplete task item
- [x] Completed task item
"#;
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<input disabled=\"\" type=\"checkbox\""));
        assert!(html.contains("checked=\"\""));
        assert!(html.contains("Incomplete task item"));
        assert!(html.contains("Completed task item"));
    }

    #[test]
    fn test_strikethrough_and_inline_code() {
        let md = "Here is ~~deleted text~~ and `const x = 42;` inline code.";
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<del>deleted text</del>"));
        assert!(html.contains("<code>const x = 42;</code>"));
    }

    #[test]
    fn test_fenced_code_blocks_with_language() {
        let md = "```rust\nfn main() {\n    println!(\"Hello Operon!\");\n}\n```";
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<pre><code class=\"language-rust\">"));
        assert!(html.contains("println!(\"Hello Operon!\");"));
        assert!(html.contains("</code></pre>"));
    }

    #[test]
    fn test_blockquotes_and_nested_lists() {
        let md = r#"
> This is a key quote from architecture doc.

1. First Item
2. Second Item
   - Sub item A
   - Sub item B
"#;
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<blockquote>"));
        assert!(html.contains("<ol>"));
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>First Item"));
        assert!(html.contains("<li>Sub item A"));
    }

    #[test]
    fn test_links_and_smart_punctuation() {
        let md = "Visit [Operon](https://github.com/lukagray-dev/operon) for \"smart quotes\" and -- dashes.";
        let html = parse_markdown_to_html(md);

        assert!(html.contains("<a href=\"https://github.com/lukagray-dev/operon\">Operon</a>"));
        // Smart punctuation turns " into “ and ”
        assert!(html.contains("“smart quotes”"));
    }

    #[test]
    fn test_streaming_incomplete_chunks() {
        // Test how partial markdown behaves (e.g. unclosed backticks or bold while streaming)
        let chunk1 = "Here is some streaming code:\n```ts\nconst greeting = 'hi';";
        let html1 = parse_markdown_to_html(chunk1);
        assert!(html1.contains("<code class=\"language-ts\">"));

        let chunk2 = "This is **partially bold text";
        let html2 = parse_markdown_to_html(chunk2);
        // Even unclosed formatting should parse deterministically without crashing
        assert!(!html2.is_empty());
    }

    #[test]
    fn test_latex_math_parsing() {
        let md =
            "Here is inline math $E = mc^2$ and display math: $$\\int_0^\\infty e^{-x} dx = 1$$";
        let html = parse_markdown_to_html(md);

        assert!(html.contains("math-inline"));
        assert!(html.contains("E = mc^2"));
        assert!(html.contains("math-display"));
        assert!(html.contains("\\int_0^\\infty e^{-x} dx = 1"));
    }

    #[tokio::test]
    async fn test_render_markdown_commands() {
        let res = render_markdown("**Hello**".to_string()).await.unwrap();
        assert_eq!(res.trim(), "<p><strong>Hello</strong></p>");

        let batch_input = vec!["# Title".to_string(), "*Item*".to_string()];
        let batch_res = render_markdown_batch(batch_input).await.unwrap();
        assert_eq!(batch_res.len(), 2);
        assert!(batch_res[0].contains("<h1>Title</h1>"));
        assert!(batch_res[1].contains("<em>Item</em>"));
    }
}
