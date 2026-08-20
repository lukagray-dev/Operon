// global_panel.rs — Global tool permissions panel component for Operon TUI.
//
// Renders the full-width tool table for global tools (Web, Subagents, Tasks, etc.).

use crate::ui::screens::permissions::state::ToolTableData;
use crate::ui::screens::permissions::tool_table::render_tool_table;
use crate::ui::theme::STYLE_ACTIVE_BORDER;
use ratatui::{layout::Rect, Frame};

/// Renders the global tools panel (full-width table).
pub fn render_global_panel(
    frame: &mut Frame,
    area: Rect,
    tools: &ToolTableData,
    selected_row: usize,
    scroll_offset: usize,
) {
    render_tool_table(
        frame,
        area,
        tools,
        selected_row,
        scroll_offset,
        " Global Tool Permissions ",
        STYLE_ACTIVE_BORDER,
    );
}
