// Directory list component
// Renders the left panel in Directory section
// Shows list of directories with their own tool permissions

use crate::ui::screens::permissions::state::DirectoryEntry;
use crate::ui::theme::{STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use ratatui::style::Style;
use ratatui::{
    layout::{Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Render the directory list panel (left side in Directory section)
/// Shows all configured directories with selection highlighting
/// Displays help text when list is empty
pub fn render_directory_list(
    frame: &mut Frame,
    area: Rect,
    directories: &[DirectoryEntry],
    selected_dir: usize,
    scroll_offset: usize,
    border_style: Style,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title("");

    let mut lines: Vec<Line> = Vec::new();

    if directories.is_empty() {
        // Empty state: show help text
        // Calculate vertical centering
        let visible_height = area.height.saturating_sub(2) as usize;
        let padding_top = visible_height / 3;

        // Add padding lines
        for _ in 0..padding_top {
            lines.push(Line::from(""));
        }

        // Centered help text
        lines.push(Line::from(Span::styled(
            "No directories added.",
            STYLE_MUTED,
        )));
        lines.push(Line::from(Span::styled(
            "Press [+] to add one.",
            STYLE_MUTED,
        )));
    } else {
        // Render directory list with selection
        for (idx, dir) in directories.iter().enumerate() {
            let is_selected = idx == selected_dir;
            let cursor = if is_selected { "> " } else { "  " };

            // Format path as string
            let path_str = dir.path.to_string_lossy();
            let display_text = format!("{}{}", cursor, path_str);

            let line = if is_selected {
                Line::from(Span::styled(display_text, STYLE_NORMAL)).style(STYLE_SELECTED)
            } else {
                Line::from(Span::styled(display_text, STYLE_NORMAL))
            };

            lines.push(line);
        }

        // Add help text at bottom
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("[+] Add", STYLE_MUTED)));
        lines.push(Line::from(Span::styled("[-] Delete", STYLE_MUTED)));
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
