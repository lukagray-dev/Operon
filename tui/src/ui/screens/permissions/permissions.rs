// Permissions screen compositor
// Main rendering logic for the permissions configuration screen
// Handles layout switching between Global (full-width) and Directory (split) sections
// Renders modals on top when open

use crate::state::AppState;
use crate::ui::screens::permissions::{
    add_directory::render_add_directory_modal,
    directory_list::render_directory_list,
    global_panel::render_global_panel,
    rule_editor::render_rule_editor_modal,
    section_tabs::build_section_tabs_title,
    state::{FocusedPanel, PermissionsSection},
    tool_table::render_tool_table,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_INACTIVE_BORDER, STYLE_MUTED};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Line,
    widgets::{Block, Borders},
    Frame,
};

/// Render the permissions configuration screen
/// Layout changes based on active section:
/// - Global: full-width tool table
/// - Directory: split layout (directory list | tool table)
///
/// Modals are rendered on top when open
pub fn render_permissions_screen(frame: &mut Frame, area: Rect, state: &mut AppState) {
    let perm_state = &mut state.permissions;

    // Build the section tabs title
    let title = build_section_tabs_title(perm_state.section);

    // Outer block with section tabs in title
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title(title);

    let outer_area = area;
    frame.render_widget(outer_block, outer_area);

    // Inner area for content (inside the outer border)
    let inner_area = outer_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 1,
    });

    // Render content based on active section
    match perm_state.section {
        PermissionsSection::Global => {
            // Global section: full-width tool table
            render_global_panel(
                frame,
                inner_area,
                &perm_state.global_tools,
                perm_state.selected_row,
                perm_state.tool_table_scroll,
            );
        }
        PermissionsSection::Directory => {
            // Directory section: split layout
            if perm_state.directories.is_empty() {
                // No directories: show directory list only (with empty state message)
                render_directory_list(
                    frame,
                    inner_area,
                    &perm_state.directories,
                    perm_state.selected_dir,
                    perm_state.dir_list_scroll,
                    STYLE_ACTIVE_BORDER,
                );
            } else {
                // Split layout: directory list (30%) | tool table (70%)
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                    .split(inner_area);

                // Determine which panel has active border
                let (dir_border, tool_border) = match perm_state.focused_panel {
                    FocusedPanel::DirList => (STYLE_ACTIVE_BORDER, STYLE_INACTIVE_BORDER),
                    FocusedPanel::ToolTable => (STYLE_INACTIVE_BORDER, STYLE_ACTIVE_BORDER),
                };

                // Render directory list (left panel)
                render_directory_list(
                    frame,
                    chunks[0],
                    &perm_state.directories,
                    perm_state.selected_dir,
                    perm_state.dir_list_scroll,
                    dir_border,
                );

                // Render tool table (right panel)
                let selected_dir_tools = &perm_state.directories[perm_state.selected_dir].tools;
                render_tool_table(
                    frame,
                    chunks[1],
                    selected_dir_tools,
                    perm_state.selected_row,
                    perm_state.tool_table_scroll,
                    "Tools",
                    tool_border,
                );
            }
        }
    }

    // Render footer with keybind hints
    let footer_area = Rect {
        x: outer_area.x,
        y: outer_area.y + outer_area.height - 1,
        width: outer_area.width,
        height: 1,
    };

    let footer_text = match perm_state.section {
        PermissionsSection::Global => {
            "[Tab] Switch section  [↑↓] Select  [Space] Edit  [Enter] Expand/Collapse"
        }
        PermissionsSection::Directory => {
            if perm_state.directories.is_empty() {
                "[Tab] Switch section  [+] Add directory"
            } else {
                "[Tab] Switch panel  [↑↓] Select  [Space] Edit  [Enter] Expand  [+] Add  [-] Delete"
            }
        }
    };

    let footer = ratatui::widgets::Paragraph::new(Line::from(footer_text))
        .style(STYLE_MUTED)
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(footer, footer_area);

    // Render modals on top if open
    if perm_state.add_dir.open {
        render_add_directory_modal(frame, outer_area, &mut perm_state.add_dir);
    }

    if perm_state.rule_editor.open {
        let tools = perm_state.active_tools();
        render_rule_editor_modal(frame, outer_area, &perm_state.rule_editor, tools);
    }
}
