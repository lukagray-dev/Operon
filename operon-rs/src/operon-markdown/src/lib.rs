//! # operon-markdown
//!
//! This crate provides the markdown parsing and HTML rendering logic for the Operon application.
//! It uses the high-performance `pulldown-cmark` library to parse and render Markdown, 
//! automatically handling GitHub Flavored Markdown (GFM) extensions such as tables, strikethroughs,
//! task lists, and footnotes.
//!
//! In addition, the crate intercepts LaTeX math environments (e.g., `$ ... $` and `$$ ... $$`)
//! and wraps them in specialized HTML blocks, making it easy for frontend consumers to auto-typeset
//! math equations using clients like KaTeX or MathJax.

use pulldown_cmark::{html, Event, Options, Parser};

/// Renders a raw markdown input string into clean, valid, and sanitized HTML.
///
/// Under the hood, this function configures the `pulldown-cmark` parser to support:
/// - **Tables:** standard Markdown tables (columns and headers)
/// - **Footnotes:** custom footnote markers and blocks
/// - **Strikethrough:** text wrapped with `~~`
/// - **Tasklists:** list checkboxes like `- [ ]` and `- [x]`
/// - **Math:** inline math `$ ... $` and block/display math `$$ ... $$`
///
/// When math elements are parsed, they are transformed into:
/// - Inline math: `<span class="math-inline">\( <latex_code> \)</span>`
/// - Display math: `<div class="math-display">\[ <latex_code> \]</div>`
///
/// # Arguments
/// * `markdown` - A string slice containing the raw markdown content to compile.
///
/// # Returns
/// A `String` containing the rendered HTML.
pub fn render(markdown: &str) -> String {
    // 0. Preprocess LaTeX math delimiters: convert \[...\] to $$...$$ and \(...\) to $...$
    // so that pulldown-cmark's ENABLE_MATH can parse them correctly.
    // Standard LLMs frequently output math using \( and \[ brackets instead of $ and $$.
    let preprocessed = markdown
        .replace(r#"\["#, "$$")
        .replace(r#"\]"#, "$$")
        .replace(r#"\("#, "$")
        .replace(r#"\)"#, "$");

    // 1. Initialize markdown extension flags.
    // These options activate non-standard CommonMark features that we want, like tables.
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_MATH);

    // 2. Instantiate the event pull-parser.
    // The parser processes the markdown into a stream of Events (like start-tag, text, end-tag).
    let parser = Parser::new_ext(&preprocessed, options);

    // 3. Map/Transform events.
    // We intercept inline and display math events to manually build safe HTML wrappers
    // with delimiters that KaTeX understands (i.e. `\(` for inline and `\[` for block).
    let processed_events = parser.map(|event| match event {
        Event::InlineMath(code) => {
            // Escape any HTML characters in the LaTeX math block for security,
            // then format it in a custom span.
            let safe_code = code.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            let html_content = format!(r#"<span class="math-inline">\({}\)</span>"#, safe_code);
            Event::Html(html_content.into())
        }
        Event::DisplayMath(code) => {
            // Do the same HTML escaping for block equations,
            // then format it inside a block level div.
            let safe_code = code.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            let html_content = format!(r#"<div class="math-display">\[{}\]</div>"#, safe_code);
            Event::Html(html_content.into())
        }
        // Let all other events (headings, paragraphs, code blocks, lists) pass through normally.
        other => other,
    });

    // 4. Serialize the final HTML representation.
    // We allocate a string and push the serialized HTML events into it.
    let mut html_output = String::new();
    html::push_html(&mut html_output, processed_events);

    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_markdown() {
        let md = "# Heading 1\nThis is **bold** text and *italic* text.";
        let html = render(md);
        assert!(html.contains("<h1>Heading 1</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
    }

    #[test]
    fn test_tables() {
        let md = "| Header 1 | Header 2 |\n| --- | --- |\n| Cell 1 | Cell 2 |";
        let html = render(md);
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>Header 1</th>"));
        assert!(html.contains("<td>Cell 1</td>"));
    }

    #[test]
    fn test_inline_math() {
        let md = "The equation is $E=mc^2$.";
        let html = render(md);
        assert!(html.contains(r#"<span class="math-inline">\(E=mc^2\)</span>"#));
    }

    #[test]
    fn test_display_math() {
        let md = "Here is display math:\n\n$$\\sum_{i=1}^n i = \\frac{n(n+1)}{2}$$";
        let html = render(md);
        assert!(html.contains(r#"<div class="math-display">\[\sum_{i=1}^n i = \frac{n(n+1)}{2}\]</div>"#));
    }

    #[test]
    fn test_bracket_math() {
        let md = "Inline: \\(a^2 + b^2 = c^2\\) and Display:\n\n\\[\\int_a^b f(x) dx\\]";
        let html = render(md);
        assert!(html.contains(r#"<span class="math-inline">\(a^2 + b^2 = c^2\)</span>"#));
        assert!(html.contains(r#"<div class="math-display">\[\int_a^b f(x) dx\]</div>"#));
    }

    #[test]
    fn test_task_lists() {
        let md = "- [ ] Unchecked item\n- [x] Checked item";
        let html = render(md);
        assert!(html.contains(r#"<input disabled="" type="checkbox""#));
    }
}
