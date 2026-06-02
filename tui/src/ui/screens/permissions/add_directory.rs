// Add directory modal
// Modal dialog for adding a new directory to the permissions list
// Allows user to input a directory path

use crate::ui::screens::permissions::state::AddDirState;
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Render the add directory modal centered over the screen
/// Shows a text input field for the directory path
/// User can confirm with Enter or cancel with Esc
pub fn render_add_directory_modal(frame: &mut Frame, area: Rect, state: &mut AddDirState) {
    // Calculate centered modal position
    // Modal width: 50% of screen width, min 40 cols
    // Modal height: 8 rows (fixed)
    let modal_width = (area.width / 2).max(40);
    let modal_height = 8;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    // Clear the area behind the modal
    frame.render_widget(Clear, modal_area);

    // Render modal block
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Add Directory");

    frame.render_widget(block, modal_area);

    // Split modal into sections
    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // "Path:" label
            Constraint::Length(3), // Input box
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Help text
        ])
        .split(inner);

    // Render "Path:" label
    let label = Paragraph::new(Line::from(Span::styled("Path:", STYLE_NORMAL)));
    frame.render_widget(label, chunks[0]);

    // Render input box with border
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER);

    let input_area = chunks[1];
    frame.render_widget(input_block, input_area);

    // Render the TextArea widget inside the input box
    let input_inner = input_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });
    frame.render_widget(&state.input, input_inner);

    // Render help text
    let help = Paragraph::new(Line::from(vec![
        Span::styled("[Enter]", STYLE_NORMAL),
        Span::styled(" Confirm   ", STYLE_MUTED),
        Span::styled("[Esc]", STYLE_NORMAL),
        Span::styled(" Cancel", STYLE_MUTED),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(help, chunks[3]);
}
