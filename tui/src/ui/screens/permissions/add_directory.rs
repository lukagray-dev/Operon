// add_directory.rs — Add directory modal dialog for Operon TUI permissions screen.
//
// Renders a centered popup dialog allowing the user to type a directory path
// and persist it into ~/.operon/config.toml via operon_rs::add_allowed_directory.

use crate::ui::screens::permissions::state::AddDirectoryState;
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Renders the add directory modal centered over the screen.
pub fn render_add_directory_modal(frame: &mut Frame, area: Rect, state: &mut AddDirectoryState) {
    let modal_width = (area.width / 2).max(44);
    let modal_height = 8;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title(" Add Allowed Directory ");

    frame.render_widget(block, modal_area);

    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Label
            Constraint::Length(3), // Input box
            Constraint::Length(1), // Help text
        ])
        .split(inner);

    let label = Paragraph::new(Line::from(Span::styled(
        "Directory Path (e.g. ~/projects or D:\\workspace):",
        STYLE_NORMAL,
    )));
    frame.render_widget(label, chunks[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER);

    let input_area = chunks[1];
    frame.render_widget(input_block, input_area);

    let input_inner = input_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(&state.input, input_inner);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("[Enter]", STYLE_NORMAL),
        Span::styled(" Confirm   ", STYLE_MUTED),
        Span::styled("[Esc]", STYLE_NORMAL),
        Span::styled(" Cancel", STYLE_MUTED),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(help, chunks[2]);
}
