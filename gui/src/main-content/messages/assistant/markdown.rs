//! Assistant message markdown parser and controller.
//!
//! Parses raw markdown strings on-the-fly as streaming tokens arrive,
//! translating them into a model of block elements that Slint can natively render.

use std::sync::OnceLock;
use pulldown_cmark::{Parser, Options, Event, Tag, TagEnd, CodeBlockKind};
use syntect::parsing::SyntaxSet;
use syntect::highlighting::ThemeSet;
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn syntect_color_to_slint(color: syntect::highlighting::Color) -> slint::Color {
    slint::Color::from_argb_u8(color.a, color.r, color.g, color.b)
}

// ─── Send-safe intermediate types ───────────────────────────────────────────
// Slint's ModelRc wraps Rc which is !Send. These plain-data structs hold the
// parsed results on background/async threads. Convert to Slint types on the UI
// thread using `to_slint_items`.

/// A single syntax-highlighted token (Send-safe, no Rc).
#[derive(Debug, Clone)]
pub struct ParsedCodeToken {
    pub text: String,
    pub color: (u8, u8, u8, u8), // (a, r, g, b)
}

/// A single line of syntax-highlighted tokens (Send-safe, no Rc).
#[derive(Debug, Clone)]
pub struct ParsedCodeLine {
    pub tokens: Vec<ParsedCodeToken>,
}

/// A single markdown block (Send-safe, no Rc).
#[derive(Debug, Clone)]
pub struct ParsedMarkdownItem {
    pub kind: String,
    pub text: String,
    pub lang: String,
    pub code_lines: Vec<ParsedCodeLine>,
    // Tool Call Fields
    pub tool_name: String,
    pub tool_title: String,
    pub tool_args: String,
    pub tool_result: String,
    pub tool_status: String, // "running", "completed", "failed"
    pub tool_is_diff: bool,
    pub tool_diff_lines: Vec<crate::main_content::tools::diff::ParsedDiffLine>,
    pub tool_added_count: i32,
    pub tool_deleted_count: i32,
    pub tool_call_id: String,
    // Permission Prompt Fields
    pub permission_id: String,
    pub permission_tool: String,
    pub permission_path: String,
    pub permission_reason: String,
    pub permission_args: String,
    pub permission_status: String, // "pending", "approved", "denied"
}

impl ParsedMarkdownItem {
    /// Constructs a standard markdown block with default empty values for tool/permission properties.
    pub fn new_default(kind: String, text: String, lang: String, code_lines: Vec<ParsedCodeLine>) -> Self {
        Self {
            kind,
            text,
            lang,
            code_lines,
            tool_name: String::new(),
            tool_title: String::new(),
            tool_args: String::new(),
            tool_result: String::new(),
            tool_status: String::new(),
            tool_is_diff: false,
            tool_diff_lines: Vec::new(),
            tool_added_count: 0,
            tool_deleted_count: 0,
            tool_call_id: String::new(),
            permission_id: String::new(),
            permission_tool: String::new(),
            permission_path: String::new(),
            permission_reason: String::new(),
            permission_args: String::new(),
            permission_status: String::new(),
        }
    }
}

/// Highlights a code block into Send-safe intermediate tokens.
fn highlight_code_sendable(code_text: &str, lang: &str) -> Vec<ParsedCodeLine> {
    let ps = get_syntax_set();
    let ts = get_theme_set();
    
    let syntax = ps.find_syntax_by_token(lang)
        .or_else(|| ps.find_syntax_by_name("Plain Text"))
        .unwrap_or_else(|| &ps.syntaxes()[0]);
        
    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);
    
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(code_text) {
        if let Ok(ranges) = h.highlight_line(line, ps) {
            let tokens: Vec<ParsedCodeToken> = ranges.into_iter().map(|(style, text)| {
                let mut clean_text = text.to_string();
                if clean_text.ends_with('\n') { clean_text.pop(); }
                if clean_text.ends_with('\r') { clean_text.pop(); }
                ParsedCodeToken {
                    text: clean_text,
                    color: (style.foreground.a, style.foreground.r, style.foreground.g, style.foreground.b),
                }
            }).collect();
            lines.push(ParsedCodeLine { tokens });
        }
    }
    lines
}

/// Parses markdown on a background/async thread into Send-safe intermediates.
/// Runs full syntax highlighting so the UI thread only needs a cheap conversion.
pub fn parse_markdown_sendable(markdown_text: &str) -> Vec<ParsedMarkdownItem> {
    let items = parse_markdown_inner_sendable(markdown_text, false);
    items
}

/// Lightweight streaming variant that parses markdown on background/async thread,
/// skipping expensive syntect highlighting to avoid freezing the UI.
pub fn parse_markdown_streaming_sendable(markdown_text: &str) -> Vec<ParsedMarkdownItem> {
    parse_markdown_inner_sendable(markdown_text, true)
}

/// Converts Send-safe parsed items into Slint MarkdownItems.
/// Must be called on the Slint UI thread (inside invoke_from_event_loop).
pub fn to_slint_items(parsed: Vec<ParsedMarkdownItem>) -> Vec<crate::MarkdownItem> {
    parsed.into_iter().map(|item| {
        let code_lines = if item.code_lines.is_empty() {
            slint::ModelRc::default()
        } else {
            let slint_lines: Vec<crate::CodeLine> = item.code_lines.into_iter().map(|line| {
                let slint_tokens: Vec<crate::CodeToken> = line.tokens.into_iter().map(|t| {
                    crate::CodeToken {
                        text: t.text.into(),
                        color: slint::Color::from_argb_u8(t.color.0, t.color.1, t.color.2, t.color.3),
                    }
                }).collect();
                crate::CodeLine {
                    tokens: slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(slint_tokens))),
                }
            }).collect();
            slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(slint_lines)))
        };
        
        let tool_diff_lines = if item.tool_diff_lines.is_empty() {
            slint::ModelRc::default()
        } else {
            let lines: Vec<crate::DiffLine> = item.tool_diff_lines.into_iter().map(|line| {
                crate::DiffLine {
                    kind: line.kind.into(),
                    text: line.text.into(),
                }
            }).collect();
            slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(lines)))
        };

        crate::MarkdownItem {
            kind: item.kind.into(),
            text: item.text.into(),
            lang: item.lang.into(),
            code_lines,
            // Tool Call Fields
            tool_name: item.tool_name.into(),
            tool_title: item.tool_title.into(),
            tool_args: item.tool_args.into(),
            tool_result: item.tool_result.into(),
            tool_status: item.tool_status.into(),
            tool_is_diff: item.tool_is_diff,
            tool_diff_lines,
            tool_added_count: item.tool_added_count,
            tool_deleted_count: item.tool_deleted_count,
            tool_call_id: item.tool_call_id.into(),
            // Permission Prompt Fields
            permission_id: item.permission_id.into(),
            permission_tool: item.permission_tool.into(),
            permission_path: item.permission_path.into(),
            permission_reason: item.permission_reason.into(),
            permission_args: item.permission_args.into(),
            permission_status: item.permission_status.into(),
        }
    }).collect()
}

/// Internal Send-safe parser with configurable highlighting.
fn parse_markdown_inner_sendable(markdown_text: &str, skip_highlighting: bool) -> Vec<ParsedMarkdownItem> {
    let mut items = Vec::new();
    
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(markdown_text, options);
    let mut state = BlockState::None;
    
    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Paragraph => {
                        if !matches!(state, BlockState::Item { .. }) {
                            flush_state_sendable(&mut state, &mut items, skip_highlighting);
                            state = BlockState::Paragraph { text: String::new() };
                        }
                    }
                    Tag::Heading { level, .. } => {
                        flush_state_sendable(&mut state, &mut items, skip_highlighting);
                        let lvl_num = match level {
                            pulldown_cmark::HeadingLevel::H1 => 1,
                            pulldown_cmark::HeadingLevel::H2 => 2,
                            pulldown_cmark::HeadingLevel::H3 => 3,
                            _ => 3,
                        };
                        state = BlockState::Heading { level: lvl_num, text: String::new() };
                    }
                    Tag::CodeBlock(kind) => {
                        flush_state_sendable(&mut state, &mut items, skip_highlighting);
                        let lang = match kind {
                            CodeBlockKind::Fenced(l) => l.to_string(),
                            CodeBlockKind::Indented => String::new(),
                        };
                        state = BlockState::CodeBlock { lang, text: String::new() };
                    }
                    Tag::Item => {
                        flush_state_sendable(&mut state, &mut items, skip_highlighting);
                        state = BlockState::Item { text: String::new() };
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                match tag {
                    TagEnd::Paragraph => {
                        if !matches!(state, BlockState::Item { .. }) {
                            flush_state_sendable(&mut state, &mut items, skip_highlighting);
                        }
                    }
                    TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item => {
                        flush_state_sendable(&mut state, &mut items, skip_highlighting);
                    }
                    _ => {}
                }
            }
            Event::Text(text) | Event::Code(text) => {
                match &mut state {
                    BlockState::Paragraph { text: t } => t.push_str(&text),
                    BlockState::Heading { text: t, .. } => t.push_str(&text),
                    BlockState::CodeBlock { text: t, .. } => t.push_str(&text),
                    BlockState::Item { text: t } => t.push_str(&text),
                    BlockState::None => {
                        state = BlockState::Paragraph { text: text.to_string() };
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                match &mut state {
                    BlockState::Paragraph { text: t } => t.push('\n'),
                    BlockState::Heading { text: t, .. } => t.push(' '),
                    BlockState::CodeBlock { text: t, .. } => t.push('\n'),
                    BlockState::Item { text: t } => t.push(' '),
                    BlockState::None => {}
                }
            }
            _ => {}
        }
    }
    
    flush_state_sendable(&mut state, &mut items, skip_highlighting);
    
    if items.is_empty() {
        items.push(ParsedMarkdownItem::new_default(
            "p".to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        ));
    }
    
    items
}

/// Flushes the current block state into a Send-safe ParsedMarkdownItem.
fn flush_state_sendable(state: &mut BlockState, items: &mut Vec<ParsedMarkdownItem>, skip_highlighting: bool) {
    match state {
        BlockState::Paragraph { text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                items.push(ParsedMarkdownItem::new_default(
                    "p".to_string(),
                    text.trim_end().to_string(),
                    String::new(),
                    Vec::new(),
                ));
            }
        }
        BlockState::Heading { level, text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let kind = match level {
                    1 => "h1",
                    2 => "h2",
                    _ => "h3",
                };
                items.push(ParsedMarkdownItem::new_default(
                    kind.to_string(),
                    text.trim().to_string(),
                    String::new(),
                    Vec::new(),
                ));
            }
        }
        BlockState::CodeBlock { lang, text } => {
            let code_lines = if skip_highlighting {
                Vec::new()
            } else {
                highlight_code_sendable(&text, &lang)
            };
            items.push(ParsedMarkdownItem::new_default(
                "code".to_string(),
                text.to_string(),
                lang.clone(),
                code_lines,
            ));
        }
        BlockState::Item { text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                items.push(ParsedMarkdownItem::new_default(
                    "bullet".to_string(),
                    text.trim().to_string(),
                    String::new(),
                    Vec::new(),
                ));
            }
        }
        BlockState::None => {}
    }
    *state = BlockState::None;
}

// ─── Slint-native types (non-Send, UI thread only) ─────────────────────────

/// Highlights a code block text with a language token and returns a vector of Slint CodeLine.
pub fn highlight_code(code_text: &str, lang: &str) -> Vec<crate::CodeLine> {
    let ps = get_syntax_set();
    let ts = get_theme_set();
    
    // Attempt to match the language token or fall back to Plain Text
    let syntax = ps.find_syntax_by_token(lang)
        .or_else(|| ps.find_syntax_by_name("Plain Text"))
        .unwrap_or_else(|| &ps.syntaxes()[0]);
        
    let theme = &ts.themes["base16-ocean.dark"];
    let mut h = HighlightLines::new(syntax, theme);
    
    let mut lines = Vec::new();
    for line in LinesWithEndings::from(code_text) {
        if let Ok(ranges) = h.highlight_line(line, ps) {
            let tokens: Vec<crate::CodeToken> = ranges.into_iter().map(|(style, text)| {
                // Slint text elements render newlines, but we strip trailing newlines in inline tokens
                // to maintain layout alignment.
                let mut clean_text = text.to_string();
                if clean_text.ends_with('\n') {
                    clean_text.pop();
                }
                if clean_text.ends_with('\r') {
                    clean_text.pop();
                }
                crate::CodeToken {
                    text: clean_text.into(),
                    color: syntect_color_to_slint(style.foreground),
                }
            }).collect();
            lines.push(crate::CodeLine {
                tokens: slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(tokens))),
            });
        }
    }
    
    lines
}

/// Helper function to create a new Slint MarkdownItem with default values for tool and permission properties.
fn new_slint_markdown_item(kind: String, text: String, lang: String, code_lines: slint::ModelRc<crate::CodeLine>) -> crate::MarkdownItem {
    crate::MarkdownItem {
        kind: kind.into(),
        text: text.into(),
        lang: lang.into(),
        code_lines,
        tool_name: "".into(),
        tool_title: "".into(),
        tool_args: "".into(),
        tool_result: "".into(),
        tool_status: "".into(),
        tool_is_diff: false,
        tool_diff_lines: slint::ModelRc::default(),
        tool_added_count: 0,
        tool_deleted_count: 0,
        tool_call_id: "".into(),
        permission_id: "".into(),
        permission_tool: "".into(),
        permission_path: "".into(),
        permission_reason: "".into(),
        permission_args: "".into(),
        permission_status: "".into(),
    }
}

#[derive(Debug, Clone, PartialEq)]
enum BlockState {
    None,
    Paragraph { text: String },
    Heading { level: u32, text: String },
    CodeBlock { lang: String, text: String },
    Item { text: String },
}

/// Parses a raw Markdown string into a vector of Slint-compatible `MarkdownItem` blocks.
/// This version runs full syntax highlighting on code blocks and should be used for
/// completed/finalized messages (e.g. loading history, after streaming finishes).
pub fn parse_markdown(markdown_text: &str) -> Vec<crate::MarkdownItem> {
    parse_markdown_inner(markdown_text, false)
}

/// Lightweight streaming variant that skips expensive syntect syntax highlighting.
/// Code blocks are rendered as plain monochrome text until the message is finalized.
/// This prevents the UI from freezing when code blocks grow incrementally.
pub fn parse_markdown_streaming(markdown_text: &str) -> Vec<crate::MarkdownItem> {
    parse_markdown_inner(markdown_text, true)
}

/// Internal parser with configurable syntax highlighting.
fn parse_markdown_inner(markdown_text: &str, skip_highlighting: bool) -> Vec<crate::MarkdownItem> {
    let mut items = Vec::new();
    
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(markdown_text, options);
    
    let mut state = BlockState::None;
    
    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Paragraph => {
                        // If we are currently collecting a list item, don't break it
                        // into a separate paragraph block.
                        if !matches!(state, BlockState::Item { .. }) {
                            flush_state(&mut state, &mut items, skip_highlighting);
                            state = BlockState::Paragraph { text: String::new() };
                        }
                    }
                    Tag::Heading { level, .. } => {
                        flush_state(&mut state, &mut items, skip_highlighting);
                        let lvl_num = match level {
                            pulldown_cmark::HeadingLevel::H1 => 1,
                            pulldown_cmark::HeadingLevel::H2 => 2,
                            pulldown_cmark::HeadingLevel::H3 => 3,
                            _ => 3,
                        };
                        state = BlockState::Heading { level: lvl_num, text: String::new() };
                    }
                    Tag::CodeBlock(kind) => {
                        flush_state(&mut state, &mut items, skip_highlighting);
                        let lang = match kind {
                            CodeBlockKind::Fenced(l) => l.to_string(),
                            CodeBlockKind::Indented => String::new(),
                        };
                        state = BlockState::CodeBlock { lang, text: String::new() };
                    }
                    Tag::Item => {
                        flush_state(&mut state, &mut items, skip_highlighting);
                        state = BlockState::Item { text: String::new() };
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                match tag {
                    TagEnd::Paragraph => {
                        // Only flush the paragraph block if we aren't in a list item
                        if !matches!(state, BlockState::Item { .. }) {
                            flush_state(&mut state, &mut items, skip_highlighting);
                        }
                    }
                    TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item => {
                        flush_state(&mut state, &mut items, skip_highlighting);
                    }
                    _ => {}
                }
            }
            Event::Text(text) | Event::Code(text) => {
                match &mut state {
                    BlockState::Paragraph { text: t } => t.push_str(&text),
                    BlockState::Heading { text: t, .. } => t.push_str(&text),
                    BlockState::CodeBlock { text: t, .. } => t.push_str(&text),
                    BlockState::Item { text: t } => t.push_str(&text),
                    BlockState::None => {
                        state = BlockState::Paragraph { text: text.to_string() };
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                match &mut state {
                    BlockState::Paragraph { text: t } => t.push('\n'),
                    BlockState::Heading { text: t, .. } => t.push(' '),
                    BlockState::CodeBlock { text: t, .. } => t.push('\n'),
                    BlockState::Item { text: t } => t.push(' '),
                    BlockState::None => {}
                }
            }
            _ => {}
        }
    }
    
    // Flush any remaining active block (crucial for streaming tokens!)
    flush_state(&mut state, &mut items, skip_highlighting);
    
    // Fallback: if list is empty, add a single empty paragraph
    if items.is_empty() {
        items.push(new_slint_markdown_item(
            "p".to_string(),
            "".to_string(),
            "".to_string(),
            slint::ModelRc::default(),
        ));
    }
    
    items
}

/// Flushes the current accumulated block state into a finished MarkdownItem.
/// When `skip_highlighting` is true, code blocks emit empty code_lines (plain text fallback).
fn flush_state(state: &mut BlockState, items: &mut Vec<crate::MarkdownItem>, skip_highlighting: bool) {
    match state {
        BlockState::Paragraph { text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                items.push(new_slint_markdown_item(
                    "p".to_string(),
                    text.trim_end().to_string(),
                    "".to_string(),
                    slint::ModelRc::default(),
                ));
            }
        }
        BlockState::Heading { level, text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let kind = match level {
                    1 => "h1",
                    2 => "h2",
                    _ => "h3",
                };
                items.push(new_slint_markdown_item(
                    kind.to_string(),
                    text.trim().to_string(),
                    "".to_string(),
                    slint::ModelRc::default(),
                ));
            }
        }
        BlockState::CodeBlock { lang, text } => {
            // Only run expensive syntect tokenization when not in streaming mode
            let code_lines = if skip_highlighting {
                slint::ModelRc::default()
            } else {
                let highlighted = highlight_code(&text, &lang);
                slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(highlighted)))
            };
            items.push(new_slint_markdown_item(
                "code".to_string(),
                text.to_string(),
                lang.clone(),
                code_lines,
            ));
        }
        BlockState::Item { text } => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                items.push(new_slint_markdown_item(
                    "bullet".to_string(),
                    text.trim().to_string(),
                    "".to_string(),
                    slint::ModelRc::default(),
                ));
            }
        }
        BlockState::None => {}
    }
    *state = BlockState::None;
}

