//! Assistant message markdown parser and controller.
//!
//! Parses raw markdown strings on-the-fly as streaming tokens arrive,
//! translating them into a model of block elements that Slint can natively render.

use pulldown_cmark::{Parser, Options, Event, Tag, TagEnd, CodeBlockKind};

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
                // If there's an active block, flush it first
                flush_state(&mut state, &mut items);
                
                match tag {
                    Tag::Paragraph => {
                        state = BlockState::Paragraph { text: String::new() };
                    }
                    Tag::Heading { level, .. } => {
                        let lvl_num = match level {
                            pulldown_cmark::HeadingLevel::H1 => 1,
                            pulldown_cmark::HeadingLevel::H2 => 2,
                            pulldown_cmark::HeadingLevel::H3 => 3,
                            _ => 3,
                        };
                        state = BlockState::Heading { level: lvl_num, text: String::new() };
                    }
                    Tag::CodeBlock(kind) => {
                        let lang = match kind {
                            CodeBlockKind::Fenced(l) => l.to_string(),
                            CodeBlockKind::Indented => String::new(),
                        };
                        state = BlockState::CodeBlock { lang, text: String::new() };
                    }
                    Tag::Item => {
                        state = BlockState::Item { text: String::new() };
                    }
                    _ => {}
                }
            }
            Event::End(tag) => {
                match tag {
                    TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item => {
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
            });
        }
        BlockState::CodeBlock { lang, text } => {
            items.push(crate::MarkdownItem {
                kind: "code".into(),
                text: text.to_string().into(),
                lang: lang.clone().into(),
            });
        }
        BlockState::Item { text } => {
            items.push(crate::MarkdownItem {
                kind: "bullet".into(),
                text: text.trim().to_string().into(),
                lang: "".into(),
            });
        }
        BlockState::None => {}
    }
    *state = BlockState::None;
}
