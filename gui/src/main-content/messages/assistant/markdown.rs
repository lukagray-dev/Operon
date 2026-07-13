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

#[derive(Debug, Clone, PartialEq)]
enum BlockState {
    None,
    Paragraph { text: String },
    Heading { level: u32, text: String },
    CodeBlock { lang: String, text: String },
    Item { text: String },
}

/// Parses a raw Markdown string into a vector of Slint-compatible `MarkdownItem` blocks.
pub fn parse_markdown(markdown_text: &str) -> Vec<crate::MarkdownItem> {
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
                            flush_state(&mut state, &mut items);
                            state = BlockState::Paragraph { text: String::new() };
                        }
                    }
                    Tag::Heading { level, .. } => {
                        flush_state(&mut state, &mut items);
                        let lvl_num = match level {
                            pulldown_cmark::HeadingLevel::H1 => 1,
                            pulldown_cmark::HeadingLevel::H2 => 2,
                            pulldown_cmark::HeadingLevel::H3 => 3,
                            _ => 3,
                        };
                        state = BlockState::Heading { level: lvl_num, text: String::new() };
                    }
                    Tag::CodeBlock(kind) => {
                        flush_state(&mut state, &mut items);
                        let lang = match kind {
                            CodeBlockKind::Fenced(l) => l.to_string(),
                            CodeBlockKind::Indented => String::new(),
                        };
                        state = BlockState::CodeBlock { lang, text: String::new() };
                    }
                    Tag::Item => {
                        flush_state(&mut state, &mut items);
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
                            flush_state(&mut state, &mut items);
                        }
                    }
                    TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item => {
                        flush_state(&mut state, &mut items);
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
    flush_state(&mut state, &mut items);
    
    // Fallback: if list is empty, add a single empty paragraph
    if items.is_empty() {
        items.push(crate::MarkdownItem {
            kind: "p".into(),
            text: "".into(),
            lang: "".into(),
            code_lines: slint::ModelRc::default(),
        });
    }
    
    items
}

fn flush_state(state: &mut BlockState, items: &mut Vec<crate::MarkdownItem>) {
    match state {
        BlockState::Paragraph { text } => {
            items.push(crate::MarkdownItem {
                kind: "p".into(),
                text: text.trim_end().to_string().into(),
                lang: "".into(),
                code_lines: slint::ModelRc::default(),
            });
        }
        BlockState::Heading { level, text } => {
            let kind = match level {
                1 => "h1",
                2 => "h2",
                _ => "h3",
            };
            items.push(crate::MarkdownItem {
                kind: kind.into(),
                text: text.trim().to_string().into(),
                lang: "".into(),
                code_lines: slint::ModelRc::default(),
            });
        }
        BlockState::CodeBlock { lang, text } => {
            let highlighted = highlight_code(&text, &lang);
            items.push(crate::MarkdownItem {
                kind: "code".into(),
                text: text.to_string().into(),
                lang: lang.clone().into(),
                code_lines: slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(highlighted))),
            });
        }
        BlockState::Item { text } => {
            items.push(crate::MarkdownItem {
                kind: "bullet".into(),
                text: text.trim().to_string().into(),
                lang: "".into(),
                code_lines: slint::ModelRc::default(),
            });
        }
        BlockState::None => {}
    }
    *state = BlockState::None;
}
