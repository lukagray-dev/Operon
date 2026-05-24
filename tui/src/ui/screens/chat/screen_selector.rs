// Screen selector inline widget
// Inline list that appears above the input box when user types '/'
// Shows all available screens with arrow key navigation

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use crate::state::screen::ActiveScreen;
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_SELECTED, STYLE_NORMAL};

/// Render the screen selector as an inline widget above the input box
/// Appears directly above input when user types '/'
/// User can navigate with arrow keys and select with Enter
pub fn render_screen_selector(frame: &mut Frame, area: Rect, selected_index: usize) {
    // Create block for screen selector
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Select Screen (↑↓: navigate, Enter: select, Esc: cancel)");

    // Get all screens
    let screens = ActiveScreen::all();

    // Create list items
    let items: Vec<ListItem> = screens
        .iter()
        .enumerate()
        .map(|(i, screen)| {
            let style = if i == selected_index {
                STYLE_SELECTED
            } else {
                STYLE_NORMAL
            };

            let prefix = if i == selected_index { "▶ " } else { "  " };
            let line = Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{}", screen), style),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    // Create list widget
    let list = List::new(items).block(block);

    frame.render_widget(list, area);
}
