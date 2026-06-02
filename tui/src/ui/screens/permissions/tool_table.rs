// Tool table component
// Renders the tool permissions table with Owner and External columns
// Shared by both Global section (full-width) and Directory section (right panel)
// Supports expanding/collapsing groups to show individual tools

use crate::ui::screens::permissions::state::ToolTableData;
use crate::ui::theme::{COLOR_MUTED, STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use ratatui::style::Style;
use ratatui::{
    layout::{Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Render the tool table with scrolling support
/// Shows tool groups with Owner and External permission columns
/// Groups can be expanded to show individual tools (indented with tree lines)
/// Selected row is highlighted with STYLE_SELECTED background
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

    // Build the line buffer for all visible rows
    let mut lines: Vec<Line> = Vec::new();

    // Header row
    lines.push(Line::from(vec![
        Span::styled("  Tool                       ", STYLE_NORMAL),
        Span::styled("Owner          ", STYLE_NORMAL),
        Span::styled("External", STYLE_NORMAL),
    ]));

    // Track the current row index (for selection highlighting)
    let mut current_row = 0;

    // Render each group and its children (if expanded)
    for group in &tools.groups {
        // Determine if this row is selected
        let is_selected = current_row == selected_row;

        // Group row
        let cursor = if is_selected { "> " } else { "  " };

        // Tool name with cursor
        let tool_name = format!("{}{:<27}", cursor, group.label);

        // Owner permission
        let owner_label = if group.is_owner_uniform() {
            group.owner.label()
        } else {
            "Custom"
        };
        let owner_style = if group.is_owner_uniform() {
            group.owner.style()
        } else {
            Style::default().fg(COLOR_MUTED)
        };

        // External permission
        let external_label = if group.is_external_uniform() {
            group.external.label()
        } else {
            "Custom"
        };
        let external_style = if group.is_external_uniform() {
            group.external.style()
        } else {
            Style::default().fg(COLOR_MUTED)
        };

        // Build the line with proper column alignment
        let line_spans = vec![
            Span::styled(tool_name, STYLE_NORMAL),
            Span::styled(format!("{:<15}", owner_label), owner_style),
            Span::styled(external_label, external_style),
        ];

        // Apply selection background to entire row
        let line = if is_selected {
            Line::from(line_spans).style(STYLE_SELECTED)
        } else {
            Line::from(line_spans)
        };

        lines.push(line);
        current_row += 1;

        // If group is expanded, render child tools with tree lines
        if group.expanded {
            let child_count = group.tools.len();
            for (child_idx, tool) in group.tools.iter().enumerate() {
                let is_last = child_idx == child_count - 1;
                let is_selected = current_row == selected_row;

                // Tree line prefix: "  ├ " for middle children, "  └ " for last child
                let tree_prefix = if is_last { "  └ " } else { "  ├ " };

                // Tool name with tree prefix and indentation
                let tool_name = format!("{}{:<23}", tree_prefix, tool.label);

                // Owner and External permissions
                let owner_label = tool.owner.label();
                let owner_style = tool.owner.style();
                let external_label = tool.external.label();
                let external_style = tool.external.style();

                // Build the line
                let line_spans = vec![
                    Span::styled(tool_name, STYLE_MUTED),
                    Span::styled(format!("{:<15}", owner_label), owner_style),
                    Span::styled(external_label, external_style),
                ];

                // Apply selection background to entire row
                let line = if is_selected {
                    Line::from(line_spans).style(STYLE_SELECTED)
                } else {
                    Line::from(line_spans)
                };

                lines.push(line);
                current_row += 1;
            }
        }
    }

    // -------------------------------------------------------------------------
    // Scroll calculation — same pattern as help.rs
    // -------------------------------------------------------------------------
    let text_width = area.width.saturating_sub(2) as usize; // subtract borders
    let visible_height = area.height.saturating_sub(2) as usize; // subtract borders

    // Count how many terminal rows each logical line occupies after wrapping
    let mut total_rows: usize = 0;
    for line in &lines {
        let line_width: usize = line.spans.iter().map(|s| s.content.len()).sum();
        if line_width == 0 {
            total_rows += 1; // empty line = one row
        } else {
            total_rows += (line_width + text_width - 1) / text_width.max(1);
        }
    }

    // Clamp scroll so we never scroll past the last line
    let max_scroll = total_rows.saturating_sub(visible_height);
    let scroll_offset = scroll_offset.min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset as u16, 0));

    frame.render_widget(paragraph, area);

    // Render scrollbar only when content overflows the visible area
    if total_rows > visible_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

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

/// Count the total number of visible rows in the tool table
/// Used for scroll bounds checking and navigation
/// Includes groups + expanded children (header is not counted as it's not selectable)
pub fn count_tool_table_rows(tools: &ToolTableData) -> usize {
    let mut count = 0; // Don't count header row

    for group in &tools.groups {
        count += 1; // group row
        if group.expanded {
            count += group.tools.len(); // child rows
        }
    }

    count
}

/// Get the group and tool indices for a given row index
/// Returns (group_idx, tool_idx) where tool_idx is None for group rows
/// Returns None if row is out of bounds
/// Row 0 = first group, row 1 = second group or first child if first group is expanded, etc.
pub fn get_row_indices(tools: &ToolTableData, row: usize) -> Option<(usize, Option<usize>)> {
    let mut current_row = 0; // Start counting from first group

    for (group_idx, group) in tools.groups.iter().enumerate() {
        if current_row == row {
            // This is a group row
            return Some((group_idx, None));
        }
        current_row += 1;

        if group.expanded {
            for tool_idx in 0..group.tools.len() {
                if current_row == row {
                    // This is a tool row
                    return Some((group_idx, Some(tool_idx)));
                }
                current_row += 1;
            }
        }
    }

    None
}
