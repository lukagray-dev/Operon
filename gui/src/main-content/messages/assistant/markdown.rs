//! Assistant message markdown parser and Slint conversion layer.
//!
//! The GUI renders markdown as native Slint components instead of HTML. This
//! module therefore keeps parsing concerns separate from UI concerns: parsing
//! produces plain Send-safe structs, and `to_slint_items` performs the cheap
//! conversion into generated Slint types on the UI thread.

use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::OnceLock;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

#[derive(Debug, Clone)]
pub struct ParsedCodeToken {
    pub text: String,
    pub color: (u8, u8, u8, u8),
}

#[derive(Debug, Clone)]
pub struct ParsedCodeLine {
    pub tokens: Vec<ParsedCodeToken>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedInlineSpan {
    pub kind: String,
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub link: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTableCell {
    pub text: String,
    pub inline_markdown: String,
    pub inline_spans: Vec<ParsedInlineSpan>,
    pub alignment: String,
    pub width_px: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTableRow {
    pub cells: Vec<ParsedTableCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMermaidNode {
    pub id: String,
    pub label: String,
    pub x: i32,
    pub y: i32,
    pub width_px: i32,
    pub shape: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMermaidEdge {
    pub path: String,
    pub arrow_path: String,
    pub label: String,
    pub label_x: i32,
    pub label_y: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedMermaidDiagram {
    pub nodes: Vec<ParsedMermaidNode>,
    pub edges: Vec<ParsedMermaidEdge>,
    pub width_px: i32,
    pub height_px: i32,
}

#[derive(Debug, Clone)]
pub struct ParsedMarkdownItem {
    pub kind: String,
    pub text: String,
    pub inline_markdown: String,
    pub inline_spans: Vec<ParsedInlineSpan>,
    pub lang: String,
    pub code_lines: Vec<ParsedCodeLine>,
    pub table_headers: Vec<ParsedTableCell>,
    pub table_rows: Vec<ParsedTableRow>,
    pub math_display: bool,
    pub mermaid_nodes: Vec<ParsedMermaidNode>,
    pub mermaid_edges: Vec<ParsedMermaidEdge>,
    pub mermaid_width_px: i32,
    pub mermaid_height_px: i32,
    pub tool_name: String,
    pub tool_title: String,
    pub tool_args: String,
    pub tool_result: String,
    pub tool_status: String,
    pub tool_is_diff: bool,
    pub tool_diff_lines: Vec<crate::main_content::tools::diff::ParsedDiffLine>,
    pub tool_added_count: i32,
    pub tool_deleted_count: i32,
    pub tool_call_id: String,
    pub permission_id: String,
    pub permission_tool: String,
    pub permission_path: String,
    pub permission_reason: String,
    pub permission_args: String,
    pub permission_status: String,
}

impl ParsedMarkdownItem {
    pub fn new_default(
        kind: String,
        text: String,
        lang: String,
        code_lines: Vec<ParsedCodeLine>,
    ) -> Self {
        Self {
            kind,
            inline_markdown: escape_inline_markdown(&text),
            inline_spans: Vec::new(),
            text,
            lang,
            code_lines,
            table_headers: Vec::new(),
            table_rows: Vec::new(),
            math_display: false,
            mermaid_nodes: Vec::new(),
            mermaid_edges: Vec::new(),
            mermaid_width_px: 0,
            mermaid_height_px: 0,
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

    fn from_inline(kind: &str, accumulator: InlineAccumulator) -> Option<Self> {
        let text = accumulator.plain.trim_end().to_string();
        if text.trim().is_empty() {
            return None;
        }

        let mut item = Self::new_default(kind.to_string(), text, String::new(), Vec::new());
        item.inline_markdown = accumulator.markdown.trim_end().to_string();
        let spans = accumulator.trimmed_spans();
        item.inline_spans = if should_render_inline_spans(&spans) {
            spans
        } else {
            Vec::new()
        };
        Some(item)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct InlineAccumulator {
    plain: String,
    markdown: String,
    spans: Vec<ParsedInlineSpan>,
    emphasis_depth: usize,
    strong_depth: usize,
    link_stack: Vec<String>,
    image_stack: Vec<String>,
}

impl InlineAccumulator {
    fn push_text(&mut self, text: &str) {
        self.plain.push_str(text);
        self.markdown.push_str(&escape_inline_markdown(text));
        self.push_text_segments(text);
    }

    fn push_code(&mut self, text: &str) {
        self.plain.push_str(text);
        // Style the inline code with a red accent color (#e06c75) using the HTML-like font tag,
        // while preserving the monospace rendering triggered by backticks.
        let styled_code = format!("<font color='#e06c75'>{}</font>", code_span_markdown(text));
        self.markdown.push_str(&styled_code);
        self.push_span("code", text);
    }

    fn push_inline_math(&mut self, text: &str) {
        let math = format!("${}$", text);
        self.plain.push_str(&math);
        self.markdown.push_str(&escape_inline_markdown(&math));
        self.push_span("math", text);
    }

    fn push_plain_break(&mut self, markdown_break: &str) {
        self.plain.push('\n');
        self.markdown.push_str(markdown_break);
        self.spans.push(ParsedInlineSpan {
            kind: "break".to_string(),
            text: String::new(),
            bold: self.strong_depth > 0,
            italic: self.emphasis_depth > 0,
            link: self.current_link(),
        });
    }

    fn push_space(&mut self) {
        self.plain.push(' ');
        self.markdown.push(' ');
        self.push_span("text", " ");
    }

    fn start_tag(&mut self, tag: &Tag<'_>) {
        match tag {
            Tag::Emphasis => {
                self.emphasis_depth += 1;
                self.markdown.push('*');
            }
            Tag::Strong => {
                self.strong_depth += 1;
                self.markdown.push_str("**");
            }
            Tag::Strikethrough => {
                self.markdown.push_str("~~");
            }
            Tag::Link { dest_url, .. } => {
                self.markdown.push('[');
                self.link_stack.push(dest_url.to_string());
            }
            Tag::Image { dest_url, .. } => {
                self.markdown.push_str("![");
                self.image_stack.push(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: &TagEnd) {
        match tag {
            TagEnd::Emphasis => {
                self.emphasis_depth = self.emphasis_depth.saturating_sub(1);
                self.markdown.push('*');
            }
            TagEnd::Strong => {
                self.strong_depth = self.strong_depth.saturating_sub(1);
                self.markdown.push_str("**");
            }
            TagEnd::Strikethrough => {
                self.markdown.push_str("~~");
            }
            TagEnd::Link => {
                let dest = self.link_stack.pop().unwrap_or_default();
                self.markdown.push_str("](");
                self.markdown.push_str(&escape_link_destination(&dest));
                self.markdown.push(')');
            }
            TagEnd::Image => {
                let dest = self.image_stack.pop().unwrap_or_default();
                self.markdown.push_str("](");
                self.markdown.push_str(&escape_link_destination(&dest));
                self.markdown.push(')');
            }
            _ => {}
        }
    }

    fn trimmed_spans(self) -> Vec<ParsedInlineSpan> {
        let mut spans = self.spans;
        trim_trailing_inline_spans(&mut spans);
        spans
    }

    fn push_text_segments(&mut self, text: &str) {
        let mut segment = String::new();
        for ch in text.chars() {
            if ch == '\n' {
                if !segment.is_empty() {
                    self.push_span("text", &segment);
                    segment.clear();
                }
                self.spans.push(ParsedInlineSpan {
                    kind: "break".to_string(),
                    text: String::new(),
                    bold: self.strong_depth > 0,
                    italic: self.emphasis_depth > 0,
                    link: self.current_link(),
                });
                continue;
            }

            segment.push(ch);
            if ch.is_whitespace() {
                self.push_span("text", &segment);
                segment.clear();
            }
        }

        if !segment.is_empty() {
            self.push_span("text", &segment);
        }
    }

    fn push_span(&mut self, kind: &str, text: &str) {
        if text.is_empty() && kind != "break" {
            return;
        }

        self.spans.push(ParsedInlineSpan {
            kind: kind.to_string(),
            text: text.to_string(),
            bold: self.strong_depth > 0,
            italic: self.emphasis_depth > 0,
            link: self.current_link(),
        });
    }

    fn current_link(&self) -> String {
        self.link_stack.last().cloned().unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TableBuilder {
    alignments: Vec<String>,
    headers: Vec<ParsedTableCell>,
    rows: Vec<ParsedTableRow>,
    current_row: Vec<ParsedTableCell>,
    current_cell: InlineAccumulator,
    in_header: bool,
    in_cell: bool,
}

impl TableBuilder {
    fn new(alignments: Vec<Alignment>) -> Self {
        Self {
            alignments: alignments.into_iter().map(alignment_to_string).collect(),
            headers: Vec::new(),
            rows: Vec::new(),
            current_row: Vec::new(),
            current_cell: InlineAccumulator::default(),
            in_header: false,
            in_cell: false,
        }
    }

    fn start_row(&mut self) {
        self.current_row.clear();
    }

    fn start_cell(&mut self) {
        self.current_cell = InlineAccumulator::default();
        self.in_cell = true;
    }

    fn end_cell(&mut self) {
        if !self.in_cell {
            return;
        }

        let column = self.current_row.len();
        let alignment = self
            .alignments
            .get(column)
            .cloned()
            .unwrap_or_else(|| "left".to_string());
        let text = self.current_cell.plain.trim().to_string();
        let inline_markdown = self.current_cell.markdown.trim().to_string();
        let mut inline_spans = self.current_cell.spans.clone();
        trim_inline_spans(&mut inline_spans);
        if !should_render_inline_spans(&inline_spans) {
            inline_spans.clear();
        }

        self.current_row.push(ParsedTableCell {
            text,
            inline_markdown,
            inline_spans,
            alignment,
            width_px: 96,
        });
        self.current_cell = InlineAccumulator::default();
        self.in_cell = false;
    }

    fn end_row(&mut self) {
        if self.current_row.is_empty() {
            return;
        }

        let row = std::mem::take(&mut self.current_row);
        if self.in_header {
            self.headers = row;
        } else {
            self.rows.push(ParsedTableRow { cells: row });
        }
    }

    fn into_item(mut self) -> Option<ParsedMarkdownItem> {
        if self.in_cell {
            self.end_cell();
        }
        if !self.current_row.is_empty() {
            self.end_row();
        }
        normalize_table_widths(&mut self.headers, &mut self.rows, &self.alignments);

        if self.headers.is_empty() && self.rows.is_empty() {
            return None;
        }

        let mut item = ParsedMarkdownItem::new_default(
            "table".to_string(),
            String::new(),
            String::new(),
            Vec::new(),
        );
        item.table_headers = self.headers;
        item.table_rows = self.rows;
        Some(item)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlockState {
    None,
    Paragraph { acc: InlineAccumulator },
    Heading { level: u32, acc: InlineAccumulator },
    CodeBlock { lang: String, text: String },
    Item { acc: InlineAccumulator },
    Quote { acc: InlineAccumulator },
    Table(TableBuilder),
}

pub fn parse_markdown_sendable(markdown_text: &str) -> Vec<ParsedMarkdownItem> {
    parse_markdown_inner_sendable(markdown_text, false)
}

pub fn parse_markdown_streaming_sendable(markdown_text: &str) -> Vec<ParsedMarkdownItem> {
    parse_markdown_inner_sendable(markdown_text, true)
}

pub fn to_slint_items(parsed: Vec<ParsedMarkdownItem>) -> Vec<crate::MarkdownItem> {
    parsed
        .into_iter()
        .map(|item| {
            let code_lines = to_model_rc(
                item.code_lines
                    .into_iter()
                    .map(to_slint_code_line)
                    .collect(),
            );
            let tool_diff_lines = to_model_rc(
                item.tool_diff_lines
                    .into_iter()
                    .map(|line| crate::DiffLine {
                        kind: line.kind.into(),
                        text: line.text.into(),
                    })
                    .collect(),
            );
            let inline_spans = to_model_rc(
                item.inline_spans
                    .into_iter()
                    .map(to_slint_inline_span)
                    .collect(),
            );
            let table_headers = to_model_rc(
                item.table_headers
                    .into_iter()
                    .map(|c| to_slint_table_cell(c, true))
                    .collect(),
            );
            let table_rows = to_model_rc(
                item.table_rows
                    .into_iter()
                    .map(to_slint_table_row)
                    .collect(),
            );
            let mermaid_nodes = to_model_rc(
                item.mermaid_nodes
                    .into_iter()
                    .map(to_slint_mermaid_node)
                    .collect(),
            );
            let mermaid_edges = to_model_rc(
                item.mermaid_edges
                    .into_iter()
                    .map(to_slint_mermaid_edge)
                    .collect(),
            );
            // For headings, we wrap the inline markdown in bold syntax ("**") so that
            // Slint's StyledText renders the entire heading text bold, while the Slint
            // heading component manages the specific font-size for h1, h2, or h3.
            let (md_str, plain_str) = match item.kind.as_str() {
                "h1" | "h2" | "h3" => (format!("**{}**", item.inline_markdown), item.text.clone()),
                _ => (item.inline_markdown.clone(), item.text.clone()),
            };

            // Pre-compile the rich text without the typing cursor.
            let styled_text = styled_text_from_markdown(&md_str, &plain_str);

            // Pre-compile the rich text with the typing cursor ('|') appended.
            // This is used by the Slint frontend to toggle the cursor blinks dynamically
            // without requiring a full re-parse from the Rust side.
            let (md_cursor_str, plain_cursor_str) = if !md_str.is_empty() {
                (format!("{}|", md_str), format!("{}|", plain_str))
            } else {
                ("".to_string(), "|".to_string())
            };
            let styled_text_with_cursor = styled_text_from_markdown(&md_cursor_str, &plain_cursor_str);

            let (svg_image, svg_valid, svg_width_px, svg_height_px) =
                svg_fields_for_item(&item.kind, &item.text);

            crate::MarkdownItem {
                kind: item.kind.into(),
                text: item.text.into(),
                styled_text,
                styled_text_with_cursor,
                inline_spans,
                lang: item.lang.into(),
                code_lines,
                table_headers,
                table_rows,
                math_display: item.math_display,
                mermaid_nodes,
                mermaid_edges,
                mermaid_width_px: item.mermaid_width_px,
                mermaid_height_px: item.mermaid_height_px,
                svg_image,
                svg_valid,
                svg_width_px,
                svg_height_px,
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
                permission_id: item.permission_id.into(),
                permission_tool: item.permission_tool.into(),
                permission_path: item.permission_path.into(),
                permission_reason: item.permission_reason.into(),
                permission_args: item.permission_args.into(),
                permission_status: item.permission_status.into(),
            }
        })
        .collect()
}

fn parse_markdown_inner_sendable(
    markdown_text: &str,
    skip_highlighting: bool,
) -> Vec<ParsedMarkdownItem> {
    let preprocessed = preprocess_markdown(markdown_text);
    if is_standalone_svg_document(&preprocessed) {
        return vec![ParsedMarkdownItem::new_default(
            "svg".to_string(),
            preprocessed.trim().to_string(),
            "svg".to_string(),
            Vec::new(),
        )];
    }

    let parser = Parser::new_ext(&preprocessed, markdown_options());
    let mut items = Vec::new();
    let mut state = BlockState::None;

    for event in parser {
        match event {
            Event::Start(tag) => handle_start_tag(&mut state, &mut items, tag, skip_highlighting),
            Event::End(tag) => handle_end_tag(&mut state, &mut items, tag, skip_highlighting),
            Event::Text(text) => push_text(&mut state, &text),
            Event::Code(text) => push_code(&mut state, &text),
            Event::InlineMath(text) => push_inline_math(&mut state, &text),
            Event::DisplayMath(text) => {
                flush_state_sendable(&mut state, &mut items, skip_highlighting);
                let mut item = ParsedMarkdownItem::new_default(
                    "math".to_string(),
                    text.to_string(),
                    String::new(),
                    Vec::new(),
                );
                item.math_display = true;
                items.push(item);
            }
            Event::SoftBreak => push_break(&mut state, false),
            Event::HardBreak => push_break(&mut state, true),
            Event::Rule => {
                flush_state_sendable(&mut state, &mut items, skip_highlighting);
                items.push(ParsedMarkdownItem::new_default(
                    "rule".to_string(),
                    String::new(),
                    String::new(),
                    Vec::new(),
                ));
            }
            Event::TaskListMarker(checked) => {
                let marker = if checked { "[x] " } else { "[ ] " };
                push_text(&mut state, marker);
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                push_html(&mut state, &mut items, &html, skip_highlighting);
            }
            Event::FootnoteReference(reference) => {
                push_text(&mut state, &format!("[^{}]", reference))
            }
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

fn handle_start_tag(
    state: &mut BlockState,
    items: &mut Vec<ParsedMarkdownItem>,
    tag: Tag<'_>,
    skip_highlighting: bool,
) {
    if let BlockState::Table(table) = state {
        match &tag {
            Tag::TableHead => {
                table.in_header = true;
                table.start_row();
            }
            Tag::TableRow => table.start_row(),
            Tag::TableCell => table.start_cell(),
            Tag::Paragraph => {}
            _ => {
                if table.in_cell {
                    table.current_cell.start_tag(&tag);
                }
            }
        }
        return;
    }

    match tag {
        Tag::Paragraph => {
            if !matches!(state, BlockState::Item { .. } | BlockState::Quote { .. }) {
                flush_state_sendable(state, items, skip_highlighting);
                *state = BlockState::Paragraph {
                    acc: InlineAccumulator::default(),
                };
            }
        }
        Tag::Heading { level, .. } => {
            flush_state_sendable(state, items, skip_highlighting);
            *state = BlockState::Heading {
                level: heading_level_to_u32(level),
                acc: InlineAccumulator::default(),
            };
        }
        Tag::CodeBlock(kind) => {
            flush_state_sendable(state, items, skip_highlighting);
            let lang = match kind {
                CodeBlockKind::Fenced(language) => language.to_string(),
                CodeBlockKind::Indented => String::new(),
            };
            *state = BlockState::CodeBlock {
                lang,
                text: String::new(),
            };
        }
        Tag::Item => {
            flush_state_sendable(state, items, skip_highlighting);
            *state = BlockState::Item {
                acc: InlineAccumulator::default(),
            };
        }
        Tag::BlockQuote(_) => {
            flush_state_sendable(state, items, skip_highlighting);
            *state = BlockState::Quote {
                acc: InlineAccumulator::default(),
            };
        }
        Tag::Table(alignments) => {
            flush_state_sendable(state, items, skip_highlighting);
            *state = BlockState::Table(TableBuilder::new(alignments));
        }
        Tag::List(_) | Tag::FootnoteDefinition(_) | Tag::HtmlBlock | Tag::MetadataBlock(_) => {}
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. } => {
            ensure_inline_accumulator(state).start_tag(&tag);
        }
        Tag::TableHead | Tag::TableRow | Tag::TableCell => {}
    }
}

fn handle_end_tag(
    state: &mut BlockState,
    items: &mut Vec<ParsedMarkdownItem>,
    tag: TagEnd,
    skip_highlighting: bool,
) {
    if let BlockState::Table(table) = state {
        match tag {
            TagEnd::TableCell => table.end_cell(),
            TagEnd::TableRow => table.end_row(),
            TagEnd::TableHead => {
                if !table.current_row.is_empty() {
                    table.end_row();
                }
                table.in_header = false;
            }
            TagEnd::Table => flush_state_sendable(state, items, skip_highlighting),
            TagEnd::Paragraph => {}
            _ => {
                if table.in_cell {
                    table.current_cell.end_tag(&tag);
                }
            }
        }
        return;
    }

    match tag {
        TagEnd::Paragraph => {
            if !matches!(state, BlockState::Item { .. } | BlockState::Quote { .. }) {
                flush_state_sendable(state, items, skip_highlighting);
            }
        }
        TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item | TagEnd::BlockQuote => {
            flush_state_sendable(state, items, skip_highlighting);
        }
        TagEnd::Emphasis
        | TagEnd::Strong
        | TagEnd::Strikethrough
        | TagEnd::Link
        | TagEnd::Image => {
            ensure_inline_accumulator(state).end_tag(&tag);
        }
        TagEnd::List(_)
        | TagEnd::FootnoteDefinition
        | TagEnd::HtmlBlock
        | TagEnd::MetadataBlock(_) => {}
        TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow | TagEnd::TableCell => {}
    }
}

fn push_text(state: &mut BlockState, text: &str) {
    match state {
        BlockState::Paragraph { acc }
        | BlockState::Heading { acc, .. }
        | BlockState::Item { acc }
        | BlockState::Quote { acc } => acc.push_text(text),
        BlockState::CodeBlock { text: code, .. } => code.push_str(text),
        BlockState::Table(table) => {
            if table.in_cell {
                table.current_cell.push_text(text);
            }
        }
        BlockState::None => {
            *state = BlockState::Paragraph {
                acc: InlineAccumulator::default(),
            };
            push_text(state, text);
        }
    }
}

fn push_html(
    state: &mut BlockState,
    items: &mut Vec<ParsedMarkdownItem>,
    html: &str,
    skip_highlighting: bool,
) {
    if looks_like_svg(html) {
        flush_state_sendable(state, items, skip_highlighting);
        items.push(ParsedMarkdownItem::new_default(
            "svg".to_string(),
            html.trim().to_string(),
            "svg".to_string(),
            Vec::new(),
        ));
    } else {
        push_text(state, html);
    }
}

fn push_code(state: &mut BlockState, text: &str) {
    match state {
        BlockState::Paragraph { acc }
        | BlockState::Heading { acc, .. }
        | BlockState::Item { acc }
        | BlockState::Quote { acc } => acc.push_code(text),
        BlockState::CodeBlock { text: code, .. } => code.push_str(text),
        BlockState::Table(table) => {
            if table.in_cell {
                table.current_cell.push_code(text);
            }
        }
        BlockState::None => {
            *state = BlockState::Paragraph {
                acc: InlineAccumulator::default(),
            };
            push_code(state, text);
        }
    }
}

fn push_inline_math(state: &mut BlockState, text: &str) {
    match state {
        BlockState::Paragraph { acc }
        | BlockState::Heading { acc, .. }
        | BlockState::Item { acc }
        | BlockState::Quote { acc } => acc.push_inline_math(text),
        BlockState::CodeBlock { text: code, .. } => code.push_str(text),
        BlockState::Table(table) => {
            if table.in_cell {
                table.current_cell.push_inline_math(text);
            }
        }
        BlockState::None => {
            *state = BlockState::Paragraph {
                acc: InlineAccumulator::default(),
            };
            push_inline_math(state, text);
        }
    }
}

fn push_break(state: &mut BlockState, hard: bool) {
    match state {
        BlockState::Paragraph { acc } | BlockState::Quote { acc } => {
            acc.push_plain_break(if hard { "  \n" } else { "\n" });
        }
        BlockState::Heading { acc, .. } | BlockState::Item { acc } => acc.push_space(),
        BlockState::CodeBlock { text, .. } => text.push('\n'),
        BlockState::Table(table) => {
            if table.in_cell {
                table.current_cell.push_space();
            }
        }
        BlockState::None => {}
    }
}

fn ensure_inline_accumulator(state: &mut BlockState) -> &mut InlineAccumulator {
    if matches!(state, BlockState::None) {
        *state = BlockState::Paragraph {
            acc: InlineAccumulator::default(),
        };
    }

    match state {
        BlockState::Paragraph { acc }
        | BlockState::Heading { acc, .. }
        | BlockState::Item { acc }
        | BlockState::Quote { acc } => acc,
        BlockState::Table(table) => &mut table.current_cell,
        BlockState::CodeBlock { .. } | BlockState::None => unreachable!(),
    }
}

fn flush_state_sendable(
    state: &mut BlockState,
    items: &mut Vec<ParsedMarkdownItem>,
    skip_highlighting: bool,
) {
    let old_state = std::mem::replace(state, BlockState::None);

    match old_state {
        BlockState::Paragraph { acc } => {
            if let Some(item) = ParsedMarkdownItem::from_inline("p", acc) {
                items.push(item);
            }
        }
        BlockState::Heading { level, acc } => {
            if let Some(mut item) = ParsedMarkdownItem::from_inline(heading_kind(level), acc) {
                item.text = item.text.trim().to_string();
                items.push(item);
            }
        }
        BlockState::CodeBlock { lang, text } => {
            let lang_token = primary_language_token(&lang);
            if is_mermaid_language(lang_token) {
                let diagram = parse_mermaid_diagram(&text);
                let mut item =
                    ParsedMarkdownItem::new_default("mermaid".to_string(), text, lang, Vec::new());
                item.mermaid_nodes = diagram.nodes;
                item.mermaid_edges = diagram.edges;
                item.mermaid_width_px = diagram.width_px;
                item.mermaid_height_px = diagram.height_px;
                items.push(item);
            } else if is_svg_language(lang_token) || looks_like_svg(&text) {
                items.push(ParsedMarkdownItem::new_default(
                    "svg".to_string(),
                    text,
                    lang,
                    Vec::new(),
                ));
            } else {
                let code_lines = if skip_highlighting {
                    Vec::new()
                } else {
                    highlight_code_sendable(&text, lang_token)
                };
                items.push(ParsedMarkdownItem::new_default(
                    "code".to_string(),
                    text,
                    lang,
                    code_lines,
                ));
            }
        }
        BlockState::Item { acc } => {
            if let Some(mut item) = ParsedMarkdownItem::from_inline("bullet", acc) {
                item.text = item.text.trim().to_string();
                item.inline_markdown = item.inline_markdown.trim().to_string();
                items.push(item);
            }
        }
        BlockState::Quote { acc } => {
            if let Some(mut item) = ParsedMarkdownItem::from_inline("quote", acc) {
                item.text = item.text.trim().to_string();
                item.inline_markdown = item.inline_markdown.trim().to_string();
                items.push(item);
            }
        }
        BlockState::Table(table) => {
            if let Some(item) = table.into_item() {
                items.push(item);
            }
        }
        BlockState::None => {}
    }
}

fn markdown_options() -> Options {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_MATH);
    options
}

fn preprocess_markdown(markdown: &str) -> String {
    markdown
        .replace(r#"\["#, "$$")
        .replace(r#"\]"#, "$$")
        .replace(r#"\("#, "$")
        .replace(r#"\)"#, "$")
}

fn highlight_code_sendable(code_text: &str, lang: &str) -> Vec<ParsedCodeLine> {
    let syntax_set = get_syntax_set();
    let theme_set = get_theme_set();
    let syntax = syntax_set
        .find_syntax_by_token(lang)
        .or_else(|| syntax_set.find_syntax_by_name("Plain Text"))
        .unwrap_or_else(|| &syntax_set.syntaxes()[0]);
    let theme = &theme_set.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for line in LinesWithEndings::from(code_text) {
        if let Ok(ranges) = highlighter.highlight_line(line, syntax_set) {
            let tokens = ranges
                .into_iter()
                .map(|(style, text)| {
                    let mut clean_text = text.to_string();
                    if clean_text.ends_with('\n') {
                        clean_text.pop();
                    }
                    if clean_text.ends_with('\r') {
                        clean_text.pop();
                    }
                    ParsedCodeToken {
                        text: clean_text,
                        color: (
                            style.foreground.a,
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        ),
                    }
                })
                .collect();
            lines.push(ParsedCodeLine { tokens });
        }
    }

    lines
}

pub fn highlight_code(code_text: &str, lang: &str) -> Vec<crate::CodeLine> {
    highlight_code_sendable(code_text, lang)
        .into_iter()
        .map(to_slint_code_line)
        .collect()
}

pub fn parse_markdown(markdown_text: &str) -> Vec<crate::MarkdownItem> {
    to_slint_items(parse_markdown_sendable(markdown_text))
}

pub fn parse_markdown_streaming(markdown_text: &str) -> Vec<crate::MarkdownItem> {
    to_slint_items(parse_markdown_streaming_sendable(markdown_text))
}

fn to_slint_code_line(line: ParsedCodeLine) -> crate::CodeLine {
    let tokens = line
        .tokens
        .into_iter()
        .map(|token| crate::CodeToken {
            text: token.text.into(),
            color: slint::Color::from_argb_u8(
                token.color.0,
                token.color.1,
                token.color.2,
                token.color.3,
            ),
        })
        .collect();

    crate::CodeLine {
        tokens: to_model_rc(tokens),
    }
}

fn to_slint_inline_span(span: ParsedInlineSpan) -> crate::InlineMarkdownSpan {
    crate::InlineMarkdownSpan {
        kind: span.kind.into(),
        text: span.text.into(),
        bold: span.bold,
        italic: span.italic,
        link: span.link.into(),
    }
}

fn to_slint_table_cell(cell: ParsedTableCell, is_header: bool) -> crate::MarkdownTableCell {
    // If this is a table header cell, wrap the parsed markdown in bold tags ("**")
    // so that Slint's StyledText natively renders the header text bold.
    let (md_str, plain_str) = if is_header {
        (format!("**{}**", cell.inline_markdown), cell.text.clone())
    } else {
        (cell.inline_markdown.clone(), cell.text.clone())
    };
    crate::MarkdownTableCell {
        text: cell.text.clone().into(),
        styled_text: styled_text_from_markdown(&md_str, &plain_str),
        spans: to_model_rc(
            cell.inline_spans
                .into_iter()
                .map(to_slint_inline_span)
                .collect(),
        ),
        alignment: cell.alignment.into(),
        width_px: cell.width_px,
    }
}

fn to_slint_table_row(row: ParsedTableRow) -> crate::MarkdownTableRow {
    crate::MarkdownTableRow {
        cells: to_model_rc(
            row.cells
                .into_iter()
                .map(|c| to_slint_table_cell(c, false))
                .collect(),
        ),
    }
}

fn to_slint_mermaid_node(node: ParsedMermaidNode) -> crate::MermaidNode {
    crate::MermaidNode {
        id: node.id.into(),
        label: node.label.into(),
        x: node.x,
        y: node.y,
        width_px: node.width_px,
        shape: node.shape.into(),
    }
}

fn to_slint_mermaid_edge(edge: ParsedMermaidEdge) -> crate::MermaidEdge {
    crate::MermaidEdge {
        path: edge.path.into(),
        arrow_path: edge.arrow_path.into(),
        label: edge.label.into(),
        label_x: edge.label_x,
        label_y: edge.label_y,
    }
}

fn svg_fields_for_item(kind: &str, source: &str) -> (slint::Image, bool, i32, i32) {
    if kind != "svg" {
        return (slint::Image::default(), false, 0, 0);
    }

    match slint::Image::load_from_svg_data(source.as_bytes()) {
        Ok(image) => {
            let size = image.size();
            (
                image,
                true,
                i32::try_from(size.width).unwrap_or(i32::MAX),
                i32::try_from(size.height).unwrap_or(i32::MAX),
            )
        }
        Err(_) => (slint::Image::default(), false, 0, 0),
    }
}

fn to_model_rc<T: Clone + 'static>(values: Vec<T>) -> slint::ModelRc<T> {
    if values.is_empty() {
        slint::ModelRc::default()
    } else {
        slint::ModelRc::from(Rc::new(slint::VecModel::from(values)))
    }
}

fn styled_text_from_markdown(markdown: &str, fallback: &str) -> slint::StyledText {
    if markdown.trim().is_empty() {
        return slint::StyledText::from_plain_text(fallback);
    }

    slint::StyledText::from_markdown(markdown)
        .unwrap_or_else(|_| slint::StyledText::from_plain_text(fallback))
}

fn heading_level_to_u32(level: HeadingLevel) -> u32 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 | HeadingLevel::H5 | HeadingLevel::H6 => 3,
    }
}

fn heading_kind(level: u32) -> &'static str {
    match level {
        1 => "h1",
        2 => "h2",
        _ => "h3",
    }
}

fn primary_language_token(language: &str) -> &str {
    language.split_whitespace().next().unwrap_or_default()
}

fn is_mermaid_language(language: &str) -> bool {
    matches!(language.to_ascii_lowercase().as_str(), "mermaid" | "mmd")
}

fn is_svg_language(language: &str) -> bool {
    matches!(
        language.to_ascii_lowercase().as_str(),
        "svg" | "xml-svg" | "image/svg+xml"
    )
}

fn looks_like_svg(source: &str) -> bool {
    let without_declaration = svg_source_without_xml_declaration(source);
    without_declaration.starts_with("<svg") && without_declaration.contains("</svg>")
}

fn is_standalone_svg_document(source: &str) -> bool {
    let without_declaration = svg_source_without_xml_declaration(source);
    without_declaration.starts_with("<svg") && without_declaration.ends_with("</svg>")
}

fn svg_source_without_xml_declaration(source: &str) -> &str {
    let trimmed = source.trim();
    if trimmed.starts_with("<?xml") {
        trimmed
            .find("?>")
            .map(|end| trimmed[end + 2..].trim_start())
            .unwrap_or(trimmed)
    } else {
        trimmed
    }
}

fn alignment_to_string(alignment: Alignment) -> String {
    match alignment {
        Alignment::Right => "right",
        Alignment::Center => "center",
        Alignment::Left | Alignment::None => "left",
    }
    .to_string()
}

fn normalize_table_widths(
    headers: &mut Vec<ParsedTableCell>,
    rows: &mut Vec<ParsedTableRow>,
    alignments: &[String],
) {
    let column_count = headers
        .len()
        .max(rows.iter().map(|row| row.cells.len()).max().unwrap_or(0))
        .max(alignments.len());

    if column_count == 0 {
        return;
    }

    pad_cells(headers, column_count, alignments);
    for row in rows.iter_mut() {
        pad_cells(&mut row.cells, column_count, alignments);
    }

    let mut widths = vec![96; column_count];
    for (column, cell) in headers.iter().enumerate() {
        widths[column] = widths[column].max(width_for_cell(&cell.text));
    }
    for row in rows.iter() {
        for (column, cell) in row.cells.iter().enumerate() {
            widths[column] = widths[column].max(width_for_cell(&cell.text));
        }
    }

    for (column, cell) in headers.iter_mut().enumerate() {
        cell.width_px = widths[column];
    }
    for row in rows.iter_mut() {
        for (column, cell) in row.cells.iter_mut().enumerate() {
            cell.width_px = widths[column];
        }
    }
}

fn pad_cells(cells: &mut Vec<ParsedTableCell>, column_count: usize, alignments: &[String]) {
    while cells.len() < column_count {
        let alignment = alignments
            .get(cells.len())
            .cloned()
            .unwrap_or_else(|| "left".to_string());
        cells.push(ParsedTableCell {
            text: String::new(),
            inline_markdown: String::new(),
            inline_spans: Vec::new(),
            alignment,
            width_px: 96,
        });
    }
}

fn width_for_cell(text: &str) -> i32 {
    let longest_line = text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    ((longest_line as i32 * 8) + 34).clamp(96, 320)
}

fn trim_inline_spans(spans: &mut Vec<ParsedInlineSpan>) {
    trim_leading_inline_spans(spans);
    trim_trailing_inline_spans(spans);
}

fn should_render_inline_spans(spans: &[ParsedInlineSpan]) -> bool {
    spans.iter().any(|span| {
        !matches!(span.kind.as_str(), "text" | "break")
            || span.bold
            || span.italic
            || !span.link.is_empty()
    })
}

fn trim_leading_inline_spans(spans: &mut Vec<ParsedInlineSpan>) {
    while let Some(first) = spans.first_mut() {
        if first.kind == "break" {
            spans.remove(0);
            continue;
        }

        let trimmed = first.text.trim_start().to_string();
        if trimmed.is_empty() {
            spans.remove(0);
        } else {
            first.text = trimmed;
            break;
        }
    }
}

fn trim_trailing_inline_spans(spans: &mut Vec<ParsedInlineSpan>) {
    while let Some(last) = spans.last_mut() {
        if last.kind == "break" {
            spans.pop();
            continue;
        }

        let trimmed = last.text.trim_end().to_string();
        if trimmed.is_empty() {
            spans.pop();
        } else {
            last.text = trimmed;
            break;
        }
    }
}

fn escape_inline_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '*' | '_' | '`' | '[' | ']' | '<' | '>' | '&' | '#' | '~' | '!' | '|' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn escape_link_destination(text: &str) -> String {
    text.replace(')', "%29").replace(' ', "%20")
}

fn code_span_markdown(code: &str) -> String {
    let longest_backtick_run = longest_run(code, '`');
    let fence = "`".repeat(longest_backtick_run + 1);
    let needs_padding = code.starts_with('`')
        || code.ends_with('`')
        || code.starts_with(' ')
        || code.ends_with(' ');

    if needs_padding {
        format!("{fence} {code} {fence}")
    } else {
        format!("{fence}{code}{fence}")
    }
}

fn longest_run(text: &str, target: char) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for ch in text.chars() {
        if ch == target {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

#[derive(Debug, Clone)]
struct MermaidEndpoint {
    id: String,
    label: String,
    shape: String,
}

#[derive(Debug, Clone)]
struct MermaidEdgeDraft {
    from: String,
    to: String,
    label: String,
}

fn parse_mermaid_diagram(source: &str) -> ParsedMermaidDiagram {
    let mut direction = "TD".to_string();
    let mut nodes: HashMap<String, MermaidEndpoint> = HashMap::new();
    let mut order = Vec::new();
    let mut edges = Vec::new();

    for raw_line in source.lines() {
        let line = raw_line.split("%%").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let first = line.split_whitespace().next().unwrap_or_default();
        if matches!(first, "graph" | "flowchart") {
            if let Some(dir) = line.split_whitespace().nth(1) {
                direction = dir.trim_end_matches(';').to_ascii_uppercase();
            }
            continue;
        }

        for statement in line
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if let Some(edge) = parse_mermaid_edge_statement(statement) {
                register_mermaid_node(&edge.0, &mut nodes, &mut order);
                register_mermaid_node(&edge.1, &mut nodes, &mut order);
                edges.push(MermaidEdgeDraft {
                    from: edge.0.id,
                    to: edge.1.id,
                    label: edge.2,
                });
            } else {
                let node = parse_mermaid_endpoint(statement);
                register_mermaid_node(&node, &mut nodes, &mut order);
            }
        }
    }

    if nodes.is_empty() {
        return ParsedMermaidDiagram::default();
    }

    layout_mermaid(direction.as_str(), nodes, order, edges)
}

fn parse_mermaid_edge_statement(
    statement: &str,
) -> Option<(MermaidEndpoint, MermaidEndpoint, String)> {
    let operator = find_mermaid_operator(statement)?;
    let from = statement[..operator.start].trim();
    let operator_text = statement[operator.start..operator.end].trim();
    let to = statement[operator.end..].trim();

    if from.is_empty() || to.is_empty() {
        return None;
    }

    Some((
        parse_mermaid_endpoint(from),
        parse_mermaid_endpoint(to),
        parse_mermaid_edge_label(operator_text),
    ))
}

#[derive(Debug, Clone, Copy)]
struct OperatorRange {
    start: usize,
    end: usize,
}

fn find_mermaid_operator(statement: &str) -> Option<OperatorRange> {
    let operators = ["-->", "==>", "-.->", "---"];
    let mut best: Option<OperatorRange> = None;

    for operator in operators {
        if let Some(index) = statement.find(operator) {
            let mut start = index;
            let mut end = index + operator.len();

            if let Some(label_start) = statement[..index].rfind("--|") {
                if statement[label_start + 3..index].contains('|') {
                    start = label_start;
                }
            }
            if let Some(after_operator) = statement[end..].strip_prefix('|') {
                if let Some(label_end) = after_operator.find('|') {
                    end += label_end + 2;
                }
            }

            let range = OperatorRange { start, end };
            best = match best {
                Some(existing) if existing.start <= range.start => Some(existing),
                _ => Some(range),
            };
        }
    }

    best
}

fn parse_mermaid_edge_label(operator: &str) -> String {
    if let Some(start) = operator.find('|') {
        if let Some(end) = operator[start + 1..].find('|') {
            return operator[start + 1..start + 1 + end].trim().to_string();
        }
    }
    String::new()
}

fn parse_mermaid_endpoint(raw: &str) -> MermaidEndpoint {
    let trimmed = raw.trim().trim_matches(';').trim();
    let shape_start = trimmed
        .find(['[', '(', '{'])
        .unwrap_or_else(|| trimmed.len());

    if shape_start == trimmed.len() {
        let id = trimmed.trim_matches('"').to_string();
        return MermaidEndpoint {
            label: id.clone(),
            id,
            shape: "rect".to_string(),
        };
    }

    let id = trimmed[..shape_start].trim().trim_matches('"').to_string();
    let opener = trimmed.as_bytes()[shape_start] as char;
    let closer = match opener {
        '[' => ']',
        '(' => ')',
        '{' => '}',
        _ => ']',
    };
    let label = trimmed[shape_start + 1..]
        .rsplit_once(closer)
        .map(|(content, _)| content)
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .to_string();
    let shape = match opener {
        '(' => "round",
        '{' => "decision",
        _ => "rect",
    };

    MermaidEndpoint {
        id: if id.is_empty() { label.clone() } else { id },
        label,
        shape: shape.to_string(),
    }
}

fn register_mermaid_node(
    node: &MermaidEndpoint,
    nodes: &mut HashMap<String, MermaidEndpoint>,
    order: &mut Vec<String>,
) {
    if node.id.is_empty() {
        return;
    }

    if !nodes.contains_key(&node.id) {
        order.push(node.id.clone());
        nodes.insert(node.id.clone(), node.clone());
    } else if let Some(existing) = nodes.get_mut(&node.id) {
        if existing.label == existing.id && !node.label.is_empty() {
            existing.label = node.label.clone();
        }
        if existing.shape == "rect" && node.shape != "rect" {
            existing.shape = node.shape.clone();
        }
    }
}

fn layout_mermaid(
    direction: &str,
    node_map: HashMap<String, MermaidEndpoint>,
    order: Vec<String>,
    edge_drafts: Vec<MermaidEdgeDraft>,
) -> ParsedMermaidDiagram {
    let horizontal = matches!(direction, "LR" | "RL");
    let reverse = matches!(direction, "RL" | "BT");
    let layers = assign_mermaid_layers(&order, &edge_drafts);
    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut rows_by_layer: HashMap<usize, usize> = HashMap::new();
    let mut positioned_nodes = Vec::new();
    let mut positions = HashMap::new();

    for id in order {
        let Some(node) = node_map.get(&id) else {
            continue;
        };
        let mut layer = *layers.get(&id).unwrap_or(&0);
        if reverse {
            layer = max_layer.saturating_sub(layer);
        }
        let row = rows_by_layer.entry(layer).or_insert(0);
        let width = width_for_mermaid_node(&node.label);
        let (x, y) = if horizontal {
            ((layer as i32 * 180) + 24, (*row as i32 * 86) + 24)
        } else {
            ((*row as i32 * 180) + 24, (layer as i32 * 86) + 24)
        };
        *row += 1;

        positioned_nodes.push(ParsedMermaidNode {
            id: node.id.clone(),
            label: if node.label.is_empty() {
                node.id.clone()
            } else {
                node.label.clone()
            },
            x,
            y,
            width_px: width,
            shape: node.shape.clone(),
        });
        positions.insert(id, (x, y, width, 40));
    }

    let mut rendered_edges = Vec::new();
    for edge in edge_drafts {
        let Some(from) = positions.get(&edge.from).copied() else {
            continue;
        };
        let Some(to) = positions.get(&edge.to).copied() else {
            continue;
        };
        rendered_edges.push(render_mermaid_edge(from, to, edge.label, horizontal));
    }

    let width_px = positioned_nodes
        .iter()
        .map(|node| node.x + node.width_px + 24)
        .max()
        .unwrap_or(240);
    let height_px = positioned_nodes
        .iter()
        .map(|node| node.y + 40 + 24)
        .max()
        .unwrap_or(120);

    ParsedMermaidDiagram {
        nodes: positioned_nodes,
        edges: rendered_edges,
        width_px,
        height_px,
    }
}

fn assign_mermaid_layers(order: &[String], edges: &[MermaidEdgeDraft]) -> HashMap<String, usize> {
    let mut incoming: HashMap<String, usize> = order.iter().map(|id| (id.clone(), 0)).collect();
    let mut outgoing: HashMap<String, Vec<String>> = HashMap::new();

    for edge in edges {
        *incoming.entry(edge.to.clone()).or_insert(0) += 1;
        outgoing
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }

    let mut layers: HashMap<String, usize> = HashMap::new();
    let mut queue: VecDeque<String> = incoming
        .iter()
        .filter_map(|(id, count)| if *count == 0 { Some(id.clone()) } else { None })
        .collect();

    if queue.is_empty() {
        queue.extend(order.iter().cloned());
    }

    while let Some(id) = queue.pop_front() {
        let current_layer = *layers.entry(id.clone()).or_insert(0);
        if let Some(targets) = outgoing.get(&id) {
            for target in targets {
                let next_layer = current_layer + 1;
                let target_layer = layers.entry(target.clone()).or_insert(next_layer);
                if *target_layer < next_layer {
                    *target_layer = next_layer;
                }
                if let Some(count) = incoming.get_mut(target) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        queue.push_back(target.clone());
                    }
                }
            }
        }
    }

    for id in order {
        layers.entry(id.clone()).or_insert(0);
    }

    layers
}

fn width_for_mermaid_node(label: &str) -> i32 {
    ((label.chars().count() as i32 * 8) + 34).clamp(96, 220)
}

fn render_mermaid_edge(
    from: (i32, i32, i32, i32),
    to: (i32, i32, i32, i32),
    label: String,
    horizontal: bool,
) -> ParsedMermaidEdge {
    let (from_x, from_y, from_width, from_height) = from;
    let (to_x, to_y, to_width, to_height) = to;

    let (start_x, start_y, end_x, end_y) = if horizontal {
        (
            from_x + from_width,
            from_y + from_height / 2,
            to_x,
            to_y + to_height / 2,
        )
    } else {
        (
            from_x + from_width / 2,
            from_y + from_height,
            to_x + to_width / 2,
            to_y,
        )
    };

    let path = format!("M {start_x} {start_y} L {end_x} {end_y}");
    let arrow_path = if horizontal {
        format!(
            "M {end_x} {end_y} L {} {} L {} {} Z",
            end_x - 7,
            end_y - 4,
            end_x - 7,
            end_y + 4
        )
    } else {
        format!(
            "M {end_x} {end_y} L {} {} L {} {} Z",
            end_x - 4,
            end_y - 7,
            end_x + 4,
            end_y - 7
        )
    };

    ParsedMermaidEdge {
        path,
        arrow_path,
        label,
        label_x: ((start_x + end_x) / 2) - 24,
        label_y: ((start_y + end_y) / 2) - 11,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_code_as_styled_markdown_payload() {
        let items = parse_markdown_sendable("Use `cargo check` before merging.");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "p");
        assert_eq!(items[0].text, "Use cargo check before merging.");
        assert!(items[0].inline_markdown.contains("`cargo check`"));
        assert!(items[0]
            .inline_spans
            .iter()
            .any(|span| span.kind == "code" && span.text == "cargo check"));
    }

    #[test]
    fn parses_gfm_table_blocks_with_alignment() {
        let md = "| Name | Score |\n| :--- | ---: |\n| Alice | 42 |\n| Bob | `7` |";
        let items = parse_markdown_sendable(md);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "table");
        assert_eq!(items[0].table_headers.len(), 2);
        assert_eq!(items[0].table_headers[0].alignment, "left");
        assert_eq!(items[0].table_headers[1].alignment, "right");
        assert_eq!(items[0].table_rows.len(), 2);
        assert!(items[0].table_rows[1].cells[1]
            .inline_markdown
            .contains('`'));
        assert!(items[0].table_rows[1].cells[1]
            .inline_spans
            .iter()
            .any(|span| span.kind == "code" && span.text == "7"));
        assert_eq!(items[0].table_headers[0].text, "Name");
    }

    #[test]
    fn parses_display_math_blocks() {
        let items = parse_markdown_sendable("Before\n\n$$E=mc^2$$\n\nAfter");

        assert_eq!(items.len(), 3);
        assert_eq!(items[1].kind, "math");
        assert_eq!(items[1].text, "E=mc^2");
        assert!(items[1].math_display);
    }

    #[test]
    fn parses_bracket_display_math_blocks() {
        let items = parse_markdown_sendable("\\[a^2 + b^2 = c^2\\]");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "math");
        assert_eq!(items[0].text, "a^2 + b^2 = c^2");
    }

    #[test]
    fn parses_mermaid_code_blocks_into_diagram_model() {
        let md = "```mermaid\ngraph TD\nA[Start] --> B{Check}\nB -->|yes| C[Done]\n```";
        let items = parse_markdown_sendable(md);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "mermaid");
        assert_eq!(items[0].mermaid_nodes.len(), 3);
        assert_eq!(items[0].mermaid_edges.len(), 2);
        assert!(items[0].mermaid_width_px > 0);
        assert!(items[0].mermaid_height_px > 0);
    }

    #[test]
    fn parses_svg_code_blocks_as_svg_items() {
        let md = "```svg\n<svg width=\"40\" height=\"20\"><rect width=\"40\" height=\"20\" /></svg>\n```";
        let items = parse_markdown_sendable(md);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "svg");
        assert_eq!(items[0].lang, "svg");
        assert!(items[0].text.contains("<svg"));
    }

    #[test]
    fn parses_raw_svg_html_as_svg_items() {
        let md = "<svg width=\"40\" height=\"20\"><rect width=\"40\" height=\"20\" /></svg>";
        let items = parse_markdown_sendable(md);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, "svg");
        assert!(items[0].text.starts_with("<svg"));
    }
}
