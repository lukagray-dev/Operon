// Global panel component
// Renders the full-width tool table for global tools (Web, Sub-agents, etc.)
// Used when Global section is active

use crate::ui::screens::permissions::state::ToolTableData;
use crate::ui::screens::permissions::tool_table::render_tool_table;
use crate::ui::theme::STYLE_ACTIVE_BORDER;
use ratatui::{layout::Rect, Frame};

/// Render the global tools panel (full-width table)
/// Shows all global tools with Owner and External permission columns
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
        "",
        STYLE_ACTIVE_BORDER,
    );
}
