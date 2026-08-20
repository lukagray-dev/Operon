// tool_table.rs — Tool permissions table component for Operon TUI.
//
// Renders the granular tool permissions table with Owner and External role columns.
// Shared by both Global section (full-width) and Directory section (right panel).
// Supports expanding/collapsing groups to show individual tools with tree branches.

use crate::ui::screens::permissions::state::ToolTableData;
use crate::ui::theme::{STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use ratatui::style::Style;
use ratatui::{
    layout::{Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Renders the tool permissions table with scrolling support and tree branches.
pub fn render_tool_table(
    frame: &mut Frame,
    area: Rect,
    tools: &ToolTableData,
    selected_row: usize,
    scroll_offset: usize,
    title: &str,
    border_style: Style,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let mut lines: Vec<Line> = Vec::new();

    // Table Header
    lines.push(Line::from(vec![
        Span::styled("  Tool / Category            ", STYLE_NORMAL),
        Span::styled("Owner           ", STYLE_NORMAL),
        Span::styled("External", STYLE_NORMAL),
    ]));

    let mut current_row = 0;

    for group in &tools.groups {
        let is_selected = current_row == selected_row;
        let cursor = if is_selected { "▶ " } else { "  " };

        let expand_glyph = if group.tools.is_empty() {
            "• "
        } else if group.expanded {
            "▼ "
        } else {
            "▶ "
        };

        let group_label = format!("{}{}{:<23}", cursor, expand_glyph, group.label);

        let owner_style = group.owner_mode.style();
        let external_style = group.external_mode.style();

        let owner_explicit_tag = if group.owner_explicit { "*" } else { " " };
        let ext_explicit_tag = if group.external_explicit { "*" } else { " " };

        let owner_display = format!("{}{:<14}", owner_explicit_tag, group.owner_mode.label());
        let ext_display = format!("{}{}", ext_explicit_tag, group.external_mode.label());

        let line_spans = vec![
            Span::styled(group_label, STYLE_NORMAL),
            Span::styled(owner_display, owner_style),
            Span::styled(ext_display, external_style),
        ];

        let line = if is_selected {
            Line::from(line_spans).style(STYLE_SELECTED)
        } else {
            Line::from(line_spans)
        };

        lines.push(line);
        current_row += 1;

        // Render child tools if expanded
        if group.expanded {
            let child_count = group.tools.len();
            for (child_idx, tool) in group.tools.iter().enumerate() {
                let is_last = child_idx == child_count - 1;
                let is_tool_selected = current_row == selected_row;

                let tool_cursor = if is_tool_selected { "▶" } else { " " };
                let tree_prefix = if is_last { " └ " } else { " ├ " };
                let tool_label = format!("{}{}{:<21}", tool_cursor, tree_prefix, tool.label);

                let t_owner_style = tool.owner_mode.style();
                let t_ext_style = tool.external_mode.style();

                let t_owner_tag = if tool.owner_explicit { "*" } else { " " };
                let t_ext_tag = if tool.external_explicit { "*" } else { " " };

                let t_owner_display = format!("{}{:<14}", t_owner_tag, tool.owner_mode.label());
                let t_ext_display = format!("{}{}", t_ext_tag, tool.external_mode.label());

                let tool_spans = vec![
                    Span::styled(tool_label, STYLE_MUTED),
                    Span::styled(t_owner_display, t_owner_style),
                    Span::styled(t_ext_display, t_ext_style),
                ];

                let tool_line = if is_tool_selected {
                    Line::from(tool_spans).style(STYLE_SELECTED)
                } else {
                    Line::from(tool_spans)
                };

                lines.push(tool_line);
                current_row += 1;
            }
        }
    }

    let text_width = area.width.saturating_sub(2) as usize;
    let visible_height = area.height.saturating_sub(2) as usize;

    let mut total_rows: usize = 0;
    for line in &lines {
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if line_width == 0 {
            total_rows += 1;
        } else {
            total_rows += (line_width + text_width - 1) / text_width.max(1);
        }
    }

    let max_scroll = total_rows.saturating_sub(visible_height);
    let scroll_offset = scroll_offset.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset as u16, 0));

    frame.render_widget(paragraph, area);

    if total_rows > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"));

        let mut scrollbar_state = ScrollbarState::new(max_scroll).position(scroll_offset);

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

/// Count the total number of selectable rows in the tool table.
pub fn count_tool_table_rows(tools: &ToolTableData) -> usize {
    let mut count = 0;
    for group in &tools.groups {
        count += 1;
        if group.expanded {
            count += group.tools.len();
        }
    }
    count
}

/// Resolves row index into (group_index, optional_tool_index).
pub fn get_row_indices(tools: &ToolTableData, row: usize) -> Option<(usize, Option<usize>)> {
    let mut current_row = 0;

    for (group_idx, group) in tools.groups.iter().enumerate() {
        if current_row == row {
            return Some((group_idx, None));
        }
        current_row += 1;

        if group.expanded {
            for tool_idx in 0..group.tools.len() {
                if current_row == row {
                    return Some((group_idx, Some(tool_idx)));
                }
                current_row += 1;
            }
        }
    }

    None
}
