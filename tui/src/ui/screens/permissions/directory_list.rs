// directory_list.rs — Directory list component for Operon TUI permissions screen.
//
// Renders the left panel in the Directory section.
// Shows all allowed directory scopes with workspace badges and selection highlights.

use crate::ui::screens::permissions::state::DirectoryEntry;
use crate::ui::theme::{COLOR_LABEL, STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use ratatui::style::Style;
use ratatui::{
    layout::{Margin, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

/// Renders the directory list panel (left side in Directory section).
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
        .title(" Allowed Directories ");

    let mut lines: Vec<Line> = Vec::new();

    if directories.is_empty() {
        let visible_height = area.height.saturating_sub(2) as usize;
        let padding_top = visible_height / 3;

        for _ in 0..padding_top {
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            "No directories added.",
            STYLE_MUTED,
        )));
        lines.push(Line::from(Span::styled(
            "Press [+] to add one.",
            STYLE_MUTED,
        )));
    } else {
        for (idx, dir) in directories.iter().enumerate() {
            let is_selected = idx == selected_dir;
            let cursor = if is_selected { "▶ " } else { "  " };

            let mut spans = vec![
                Span::styled(cursor, STYLE_NORMAL),
                Span::styled(&dir.path, STYLE_NORMAL),
            ];

            if dir.is_workspace {
                spans.push(Span::styled(
                    " (workspace)",
                    Style::default().fg(COLOR_LABEL),
                ));
            }

            let line = if is_selected {
                Line::from(spans).style(STYLE_SELECTED)
            } else {
                Line::from(spans)
            };

            lines.push(line);
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("[+] Add directory", STYLE_MUTED)));
        lines.push(Line::from(Span::styled(
            "[-] Remove directory",
            STYLE_MUTED,
        )));
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
