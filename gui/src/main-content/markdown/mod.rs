// =============================================================================
// Markdown Parser & Engine (`src/main-content/markdown/mod.rs`)
// =============================================================================
// Main Rust backend engine for parsing Markdown documents into Slint `MarkdownElement` models
// using the high-performance `pulldown-cmark` CommonMark / GFM parser.

pub mod blockquote;
pub mod bold;
pub mod code;
pub mod code_block;
pub mod heading;
pub mod italic;
pub mod link;
pub mod list;
pub mod math;
pub mod mermaid;
pub mod paragraph;
pub mod rule;
pub mod spacer;
pub mod table;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use slint::{ModelRc, SharedString, StyledText, VecModel};

use crate::{MarkdownBlockType, MarkdownElement, MarkdownTableCell, MarkdownTableRow};

/// Plain-Rust thread-safe intermediate item for block accumulation across async events
#[derive(Debug, Clone)]
pub struct ParsedMarkdownItem {
    pub kind: String, // "text", "thinking", "tool", "work_group", "permission"
    pub text: String,
    pub rich_text: String,
    pub level: i32,
    pub is_ordered: bool,
    pub prefix: String,
    pub is_task: bool,
    pub is_checked: bool,
    pub depth: i32,
    pub language: String,

    // Thinking Card fields
    pub thinking_text: String,
    pub is_thinking_active: bool,

    // Tool Call fields
    pub tool_name: String,
    pub tool_title: String,
    pub tool_args: String,
    pub tool_result: String,
    pub tool_status: String,
    pub tool_call_id: String,
    pub item_expanded: bool,

    // Collapsed turn activity summary fields
    pub work_group_items: Vec<ParsedMarkdownItem>,
    pub work_group_active: bool,
    pub work_group_elapsed_secs: i32,
    pub work_group_expanded: bool,
    pub work_group_summary: String,

    // Permission Prompt fields
    pub permission_id: String,
    pub permission_tool: String,
    pub permission_path: String,
    pub permission_reason: String,
    pub permission_args: String,
    pub permission_status: String,
}

impl ParsedMarkdownItem {
    pub fn new_default(kind: String, text: String) -> Self {
        Self {
            kind,
            text,
            rich_text: String::new(),
            level: 0,
            is_ordered: false,
            prefix: String::new(),
            is_task: false,
            is_checked: false,
            depth: 0,
            language: String::new(),
            thinking_text: String::new(),
            is_thinking_active: false,
            tool_name: String::new(),
            tool_title: String::new(),
            tool_args: String::new(),
            tool_result: String::new(),
            tool_status: String::new(),
            tool_call_id: String::new(),
            item_expanded: false,
            work_group_items: Vec::new(),
            work_group_active: false,
            work_group_elapsed_secs: 0,
            work_group_expanded: false,
            work_group_summary: String::new(),
            permission_id: String::new(),
            permission_tool: String::new(),
            permission_path: String::new(),
            permission_reason: String::new(),
            permission_args: String::new(),
            permission_status: String::new(),
        }
    }
}

pub fn default_markdown_element(block_type: crate::MarkdownBlockType) -> crate::MarkdownElement {
    crate::MarkdownElement {
        block_type,
        text: Default::default(),
        rich_text: Default::default(),
        level: 0,
        is_ordered: false,
        prefix: Default::default(),
        is_task: false,
        is_checked: false,
        depth: 0,
        rows: Default::default(),
        language: Default::default(),
        diagram_image: Default::default(),
        thinking_text: Default::default(),
        is_thinking_active: false,
        tool_name: Default::default(),
        tool_title: Default::default(),
        tool_args: Default::default(),
        tool_result: Default::default(),
        tool_status: Default::default(),
        tool_call_id: Default::default(),
        work_group_items: Default::default(),
        work_group_active: false,
        work_group_elapsed_secs: 0,
        work_group_expanded: false,
        work_group_summary: Default::default(),
        permission_id: Default::default(),
        permission_tool: Default::default(),
        permission_path: Default::default(),
        permission_reason: Default::default(),
        permission_args: Default::default(),
        permission_status: Default::default(),
    }
}

fn parsed_item_to_work_group_item(item: ParsedMarkdownItem) -> crate::WorkGroupItem {
    crate::WorkGroupItem {
        kind: item.kind.into(),
        thinking_text: item.thinking_text.into(),
        tool_name: item.tool_name.into(),
        tool_title: item.tool_title.into(),
        tool_args: item.tool_args.into(),
        tool_result: item.tool_result.into(),
        tool_status: item.tool_status.into(),
        tool_call_id: item.tool_call_id.into(),
        item_expanded: item.item_expanded,
    }
}

fn convert_work_group(mut item: ParsedMarkdownItem) -> crate::MarkdownElement {
    let mut elem = default_markdown_element(crate::MarkdownBlockType::WorkGroup);
    elem.work_group_active = item.work_group_active;
    elem.work_group_elapsed_secs = item.work_group_elapsed_secs;
    elem.work_group_expanded = item.work_group_expanded;
    elem.work_group_summary = item.work_group_summary.into();

    let work_items: Vec<crate::WorkGroupItem> = item
        .work_group_items
        .drain(..)
        .map(parsed_item_to_work_group_item)
        .collect();
    elem.work_group_items = ModelRc::new(VecModel::from(work_items));
    elem
}

fn convert_legacy_work_items(items: Vec<ParsedMarkdownItem>) -> crate::MarkdownElement {
    let active = items.iter().any(|item| {
        item.is_thinking_active || (item.kind == "tool" && item.tool_status == "running")
    });
    let summary = crate::main_content::reasoning::build_work_summary(&items);

    let mut group = ParsedMarkdownItem::new_default("work_group".to_string(), String::new());
    group.work_group_items = items;
    group.work_group_active = active;
    group.work_group_elapsed_secs = 0;
    group.work_group_summary = summary;
    convert_work_group(group)
}

/// Converts plain-Rust `ParsedMarkdownItem` blocks into Slint `MarkdownElement` blocks on the UI thread.
pub fn to_slint_elements(items: Vec<ParsedMarkdownItem>) -> Vec<crate::MarkdownElement> {
    let mut result = Vec::new();
    let mut pending_legacy_work_items = Vec::new();

    for item in items {
        match item.kind.as_str() {
            "thinking" | "tool" => {
                pending_legacy_work_items.push(item);
            }
            "work_group" => {
                if !pending_legacy_work_items.is_empty() {
                    result.push(convert_legacy_work_items(std::mem::take(
                        &mut pending_legacy_work_items,
                    )));
                }
                result.push(convert_work_group(item));
            }
            _ => {
                let is_whitespace_text = item.kind == "text" && item.text.trim().is_empty();
                if !is_whitespace_text {
                    if !pending_legacy_work_items.is_empty() {
                        result.push(convert_legacy_work_items(std::mem::take(
                            &mut pending_legacy_work_items,
                        )));
                    }
                    let parsed = parse_markdown(&item.text);
                    result.extend(parsed);
                }
            }
        }
    }

    if !pending_legacy_work_items.is_empty() {
        result.push(convert_legacy_work_items(pending_legacy_work_items));
    }

    // Remove any Spacer element that immediately follows a WorkGroup
    let mut i = 0;
    while i + 1 < result.len() {
        if result[i].block_type == crate::MarkdownBlockType::WorkGroup
            && result[i + 1].block_type == crate::MarkdownBlockType::Spacer
        {
            result.remove(i + 1);
        } else {
            i += 1;
        }
    }

    result
}

#[derive(Debug, Clone)]
struct ListState {
    is_ordered: bool,
    current_index: i32,
}

#[derive(Debug, Clone)]
struct ItemFrame {
    element_index: usize,
    depth: i32,
    is_ordered: bool,
    prefix: String,
    is_task: bool,
    is_checked: bool,
    rich_md: String,
    plain_text: String,
}

#[derive(Default)]
struct TableState {
    alignments: Vec<i32>,
    rows: Vec<MarkdownTableRow>,
    current_row_cells: Vec<MarkdownTableCell>,
    is_current_row_header: bool,
    current_cell_align_idx: usize,
    in_table: bool,
    in_table_head: bool,
    in_table_cell: bool,
}

impl TableState {
    fn flush_current_row(&mut self) {
        if !self.current_row_cells.is_empty() {
            let cells_model: ModelRc<MarkdownTableCell> =
                ModelRc::new(VecModel::from(self.current_row_cells.clone()));
            self.rows.push(MarkdownTableRow {
                cells: cells_model,
                is_header: self.is_current_row_header,
            });
            self.current_row_cells.clear();
        }
    }
}

#[derive(Default)]
struct CodeBlockState {
    in_code_block: bool,
    language: String,
    code_buf: String,
}

fn to_styled_text(md: &str) -> StyledText {
    StyledText::from_markdown(md).unwrap_or_default()
}

/// Parses raw Markdown string into a vector of Slint `MarkdownElement` blocks.
pub fn parse_markdown(markdown_text: &str) -> Vec<MarkdownElement> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_MATH);

    let parser = Parser::new_ext(markdown_text, options);
    let mut elements = Vec::new();

    let mut current_heading_level: i32 = 1;
    let mut is_in_heading = false;
    let mut is_in_paragraph = false;
    let mut blockquote_depth: usize = 0;
    let mut max_blockquote_depth: usize = 0;
    let mut current_link_url = String::new();
    let mut is_in_link = false;

    let mut global_rich_md = String::new();

    let mut list_stack: Vec<ListState> = Vec::new();
    let mut item_stack: Vec<ItemFrame> = Vec::new();

    let mut table_state = TableState::default();
    let mut code_block_state = CodeBlockState::default();

    let mut last_top_level_pos: usize = 0;

    let check_and_insert_spacer =
        |elements: &mut Vec<MarkdownElement>, last_pos: usize, current_start: usize| {
            if current_start > last_pos && last_pos < markdown_text.len() {
                let slice = &markdown_text[last_pos..current_start.min(markdown_text.len())];
                let blank_lines = spacer::calculate_blank_lines(slice);
                if blank_lines > 0 {
                    let mut elem = default_markdown_element(MarkdownBlockType::Spacer);
                    elem.level = blank_lines;
                    elements.push(elem);
                }
            }
        };

    for (event, range) in parser.into_offset_iter() {
        match event {
            // ---- BlockQuotes ----
            Event::Start(Tag::BlockQuote(_)) => {
                if blockquote_depth == 0
                    && list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                    global_rich_md.clear();
                    max_blockquote_depth = 0;
                }
                blockquote_depth += 1;
                if blockquote_depth > max_blockquote_depth {
                    max_blockquote_depth = blockquote_depth;
                }
            }
            Event::End(TagEnd::BlockQuote) => {
                if blockquote_depth > 0 {
                    blockquote_depth -= 1;
                    if blockquote_depth == 0 {
                        let md = blockquote::clean_blockquote_text(&global_rich_md);
                        if !md.is_empty() {
                            let mut elem = default_markdown_element(MarkdownBlockType::BlockQuote);
                            elem.text = SharedString::from(md.clone());
                            elem.rich_text = to_styled_text(&md);
                            elem.depth = (max_blockquote_depth.saturating_sub(1)) as i32;
                            elements.push(elem);
                            last_top_level_pos = range.end;
                        }
                        global_rich_md.clear();
                    }
                }
            }

            // ---- Headings ----
            Event::Start(Tag::Heading { level, .. }) => {
                if list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                }
                is_in_heading = true;
                current_heading_level = heading::heading_level_to_int(level);
                global_rich_md.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if is_in_heading {
                    is_in_heading = false;
                    let md = global_rich_md.trim().to_string();
                    if !md.is_empty() {
                        let mut elem = default_markdown_element(MarkdownBlockType::Heading);
                        elem.text = SharedString::from(md.clone());
                        elem.rich_text = to_styled_text(&md);
                        elem.level = current_heading_level;
                        elements.push(elem);
                        last_top_level_pos = range.end;
                    }
                    global_rich_md.clear();
                }
            }

            // ---- Paragraphs ----
            Event::Start(Tag::Paragraph) => {
                if list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                    is_in_paragraph = true;
                    global_rich_md.clear();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if is_in_paragraph
                    && list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    is_in_paragraph = false;
                    let md = paragraph::clean_paragraph_text(global_rich_md.trim());
                    if !md.is_empty() {
                        let mut elem = default_markdown_element(MarkdownBlockType::Paragraph);
                        elem.text = SharedString::from(md.clone());
                        elem.rich_text = to_styled_text(&md);
                        elements.push(elem);
                        last_top_level_pos = range.end;
                    }
                    global_rich_md.clear();
                }
            }

            // ---- Horizontal Rule (---) ----
            Event::Rule => {
                if list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                    let mut elem = default_markdown_element(MarkdownBlockType::Rule);
                    elem.text = SharedString::from(rule::format_rule());
                    elements.push(elem);
                    last_top_level_pos = range.end;
                }
            }

            // ---- Code Blocks & Mermaid Diagrams ----
            Event::Start(Tag::CodeBlock(kind)) => {
                if list_stack.is_empty() && !table_state.in_table && blockquote_depth == 0 {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                }
                code_block_state.in_code_block = true;
                code_block_state.language = match kind {
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code_block_state.code_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if code_block_state.in_code_block {
                    code_block_state.in_code_block = false;

                    if code_block_state.language.eq_ignore_ascii_case("mermaid") {
                        let diagram_img =
                            mermaid::render_mermaid_diagram(&code_block_state.code_buf);
                        let mut elem = default_markdown_element(MarkdownBlockType::Mermaid);
                        elem.text = SharedString::from(code_block_state.code_buf.clone());
                        elem.language = SharedString::from("MERMAID");
                        elem.diagram_image = diagram_img;
                        elements.push(elem);
                    } else {
                        let highlighted_md = code_block::highlight_code_block(
                            &code_block_state.code_buf,
                            &code_block_state.language,
                        );
                        let lang_label = if code_block_state.language.is_empty() {
                            String::new()
                        } else {
                            code_block_state.language.to_uppercase()
                        };
                        let mut elem = default_markdown_element(MarkdownBlockType::CodeBlock);
                        elem.text = SharedString::from(code_block_state.code_buf.clone());
                        elem.rich_text = to_styled_text(&highlighted_md);
                        elem.language = SharedString::from(lang_label);
                        elements.push(elem);
                    }
                    last_top_level_pos = range.end;
                }
            }

            // ---- Links ----
            Event::Start(Tag::Link { dest_url, .. }) => {
                is_in_link = true;
                current_link_url = dest_url.to_string();
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push('[');
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push('[');
                }
            }
            Event::End(TagEnd::Link) => {
                if is_in_link {
                    is_in_link = false;
                    let sanitized = link::sanitize_url(&current_link_url);
                    let close_link = format!("]({})", sanitized);
                    if let Some(frame) = item_stack.last_mut() {
                        frame.rich_md.push_str(&close_link);
                    } else if is_in_heading
                        || is_in_paragraph
                        || table_state.in_table_cell
                        || blockquote_depth > 0
                    {
                        global_rich_md.push_str(&close_link);
                    }
                    current_link_url.clear();
                }
            }

            // ---- Math Events ($...$ and $$...$$) ----
            Event::InlineMath(math_expr) => {
                let formatted = math::format_inline_math(&math_expr);
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str(&formatted);
                    frame.plain_text.push_str(&math_expr);
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str(&formatted);
                }
            }
            Event::DisplayMath(math_expr) => {
                if list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                    let display_str = math::format_display_math(&math_expr);
                    let mut elem = default_markdown_element(MarkdownBlockType::DisplayMath);
                    elem.text = SharedString::from(math_expr.to_string());
                    elem.rich_text = to_styled_text(&display_str);
                    elements.push(elem);
                    last_top_level_pos = range.end;
                }
            }

            // ---- Tables ----
            Event::Start(Tag::Table(alignments)) => {
                if list_stack.is_empty() && !code_block_state.in_code_block && blockquote_depth == 0
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                }
                table_state.in_table = true;
                table_state.alignments = alignments
                    .into_iter()
                    .map(table::alignment_to_int)
                    .collect();
                table_state.rows.clear();
            }
            Event::End(TagEnd::Table) => {
                if table_state.in_table {
                    table_state.flush_current_row();
                    table_state.in_table = false;
                    let rows_model: ModelRc<MarkdownTableRow> =
                        ModelRc::new(VecModel::from(table_state.rows.clone()));
                    let mut elem = default_markdown_element(MarkdownBlockType::Table);
                    elem.rows = rows_model;
                    elements.push(elem);
                    last_top_level_pos = range.end;
                }
            }

            Event::Start(Tag::TableHead) => {
                table_state.in_table_head = true;
                table_state.is_current_row_header = true;
                table_state.current_row_cells.clear();
                table_state.current_cell_align_idx = 0;
            }
            Event::End(TagEnd::TableHead) => {
                table_state.flush_current_row();
                table_state.in_table_head = false;
            }

            Event::Start(Tag::TableRow) => {
                table_state.current_row_cells.clear();
                table_state.is_current_row_header = table_state.in_table_head;
                table_state.current_cell_align_idx = 0;
            }
            Event::End(TagEnd::TableRow) => {
                table_state.flush_current_row();
            }

            Event::Start(Tag::TableCell) => {
                table_state.in_table_cell = true;
                global_rich_md.clear();
            }
            Event::End(TagEnd::TableCell) => {
                table_state.in_table_cell = false;
                let align = table_state
                    .alignments
                    .get(table_state.current_cell_align_idx)
                    .copied()
                    .unwrap_or(0);
                table_state.current_cell_align_idx += 1;

                let md = global_rich_md.trim().to_string();
                table_state.current_row_cells.push(MarkdownTableCell {
                    text: SharedString::from(md.clone()),
                    rich_text: to_styled_text(&md),
                    alignment: align,
                    is_header: table_state.is_current_row_header,
                });
                global_rich_md.clear();
            }

            // ---- Lists ----
            Event::Start(Tag::List(start_number)) => {
                if list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    check_and_insert_spacer(&mut elements, last_top_level_pos, range.start);
                }
                let is_ordered = start_number.is_some();
                let current_index = start_number.unwrap_or(1) as i32;
                list_stack.push(ListState {
                    is_ordered,
                    current_index,
                });
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                if list_stack.is_empty()
                    && !table_state.in_table
                    && !code_block_state.in_code_block
                    && blockquote_depth == 0
                {
                    last_top_level_pos = range.end;
                }
            }

            // ---- List Items ----
            Event::Start(Tag::Item) => {
                let depth = (list_stack.len() as i32 - 1).max(0);
                let (is_ordered, prefix) = if let Some(last) = list_stack.last_mut() {
                    let is_ord = last.is_ordered;
                    let pref = if is_ord {
                        let p = list::format_ordered_prefix(last.current_index, depth);
                        last.current_index += 1;
                        p
                    } else {
                        String::new()
                    };
                    (is_ord, pref)
                } else {
                    (false, String::new())
                };

                let element_index = elements.len();
                let mut elem = default_markdown_element(MarkdownBlockType::ListItem);
                elem.is_ordered = is_ordered;
                elem.prefix = SharedString::from(prefix.clone());
                elem.depth = depth;
                elements.push(elem);

                item_stack.push(ItemFrame {
                    element_index,
                    depth,
                    is_ordered,
                    prefix,
                    is_task: false,
                    is_checked: false,
                    rich_md: String::new(),
                    plain_text: String::new(),
                });
            }
            Event::End(TagEnd::Item) => {
                if let Some(frame) = item_stack.pop() {
                    let md = frame.rich_md.trim().to_string();
                    if frame.element_index < elements.len() {
                        let mut elem = default_markdown_element(MarkdownBlockType::ListItem);
                        elem.text = SharedString::from(frame.plain_text.trim());
                        elem.rich_text = to_styled_text(&md);
                        elem.is_ordered = frame.is_ordered;
                        elem.prefix = SharedString::from(frame.prefix);
                        elem.is_task = frame.is_task;
                        elem.is_checked = frame.is_checked;
                        elem.depth = frame.depth;
                        elements[frame.element_index] = elem;
                    }
                }
            }

            Event::TaskListMarker(checked) => {
                if let Some(frame) = item_stack.last_mut() {
                    frame.is_task = true;
                    frame.is_checked = checked;
                }
            }

            Event::Start(Tag::Strong) => {
                let marker = bold::open_bold_tag();
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str(marker);
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str(marker);
                }
            }
            Event::End(TagEnd::Strong) => {
                let marker = bold::close_bold_tag();
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str(marker);
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str(marker);
                }
            }

            Event::Start(Tag::Emphasis) => {
                let marker = italic::open_italic_tag();
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str(marker);
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str(marker);
                }
            }
            Event::End(TagEnd::Emphasis) => {
                let marker = italic::close_italic_tag();
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str(marker);
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str(marker);
                }
            }

            Event::Start(Tag::Strikethrough) => {
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str("~~");
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str("~~");
                }
            }
            Event::End(TagEnd::Strikethrough) => {
                if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str("~~");
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str("~~");
                }
            }

            Event::Text(text) => {
                if code_block_state.in_code_block {
                    code_block_state.code_buf.push_str(&text);
                } else if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push_str(&text);
                    frame.plain_text.push_str(&text);
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push_str(&text);
                }
            }
            Event::Code(code_str) => {
                if code_block_state.in_code_block {
                    code_block_state.code_buf.push_str(&code_str);
                } else {
                    let formatted = code::format_inline_code(&code_str);
                    if let Some(frame) = item_stack.last_mut() {
                        frame.rich_md.push_str(&formatted);
                        frame.plain_text.push_str(&code_str);
                    } else if is_in_heading
                        || is_in_paragraph
                        || table_state.in_table_cell
                        || blockquote_depth > 0
                    {
                        global_rich_md.push_str(&formatted);
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if code_block_state.in_code_block {
                    code_block_state.code_buf.push('\n');
                } else if let Some(frame) = item_stack.last_mut() {
                    frame.rich_md.push('\n');
                    frame.plain_text.push('\n');
                } else if is_in_heading
                    || is_in_paragraph
                    || table_state.in_table_cell
                    || blockquote_depth > 0
                {
                    global_rich_md.push('\n');
                }
            }
            _ => {}
        }
    }

    check_and_insert_spacer(&mut elements, last_top_level_pos, markdown_text.len());

    elements.retain(|e| {
        e.block_type == MarkdownBlockType::ListItem
            || e.block_type == MarkdownBlockType::Rule
            || e.block_type == MarkdownBlockType::Spacer
            || e.block_type == MarkdownBlockType::Table
            || e.block_type == MarkdownBlockType::CodeBlock
            || e.block_type == MarkdownBlockType::DisplayMath
            || e.block_type == MarkdownBlockType::BlockQuote
            || e.block_type == MarkdownBlockType::Mermaid
            || e.block_type == MarkdownBlockType::WorkGroup
            || !e.text.is_empty()
    });

    // Remove leading Spacers to prevent top gap above first paragraph
    while elements.first().map_or(false, |e| e.block_type == MarkdownBlockType::Spacer) {
        elements.remove(0);
    }
    // Remove trailing Spacers to prevent bottom gap
    while elements.last().map_or(false, |e| e.block_type == MarkdownBlockType::Spacer) {
        elements.pop();
    }

    elements
}
