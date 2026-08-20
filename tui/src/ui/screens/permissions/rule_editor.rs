// rule_editor.rs — Permission rule editor modal component for Operon TUI.
//
// Modal dialog for editing a single permission cell (tool/group × role).
// Allows selecting Allow, Ask, or Deny for Owner or External roles.

use crate::ui::screens::permissions::state::{EditRole, PermissionMode, RuleEditorState, ToolTableData};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_NORMAL, STYLE_SELECTED};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Renders the rule editor modal centered over the screen.
pub fn render_rule_editor_modal(
    frame: &mut Frame,
    area: Rect,
    state: &RuleEditorState,
    tools: &ToolTableData,
) {
    let modal_width = (area.width / 2).max(44);
    let modal_height = 12;

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
        .title(" Edit Tool Permission ");

    frame.render_widget(block, modal_area);

    let inner = modal_area.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tool label
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Role tabs (Owner • External)
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Allow option
            Constraint::Length(1), // Ask option
            Constraint::Length(1), // Deny option
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Help text
        ])
        .split(inner);

    if state.group_idx >= tools.groups.len() {
        return;
    }

    let group = &tools.groups[state.group_idx];
    let tool_name = if let Some(tool_idx) = state.tool_idx {
        if tool_idx < group.tools.len() {
            format!("{} → {}", group.label, group.tools[tool_idx].label)
        } else {
            group.label.clone()
        }
    } else {
        group.label.clone()
    };

    let tool_label = Paragraph::new(Line::from(vec![
        Span::styled("Target: ", STYLE_MUTED),
        Span::styled(tool_name, STYLE_NORMAL),
    ]));
    frame.render_widget(tool_label, chunks[0]);

    let (owner_style, external_style) = match state.role {
        EditRole::Owner => (
            ratatui::style::Style::default()
                .fg(crate::ui::theme::COLOR_ACCENT)
                .add_modifier(ratatui::style::Modifier::BOLD),
            STYLE_MUTED,
        ),
        EditRole::External => (
            STYLE_MUTED,
            ratatui::style::Style::default()
                .fg(crate::ui::theme::COLOR_ACCENT)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
    };

    let role_tabs = Paragraph::new(Line::from(vec![
        Span::styled("Role:   ", STYLE_MUTED),
        Span::styled("[Owner]", owner_style),
        Span::styled(" • ", STYLE_MUTED),
        Span::styled("[External]", external_style),
        Span::styled("  (Press Tab to switch)", STYLE_MUTED),
    ]));
    frame.render_widget(role_tabs, chunks[2]);

    let modes = [
        PermissionMode::Allow,
        PermissionMode::Ask,
        PermissionMode::Deny,
    ];

    for (idx, mode) in modes.iter().enumerate() {
        let is_selected = *mode == state.selected_mode;
        let radio = if is_selected { "(•) " } else { "( ) " };
        let label = mode.label();

        let line = Line::from(vec![
            Span::styled(radio, STYLE_NORMAL),
            Span::styled(label, mode.style()),
        ]);

        let paragraph = if is_selected {
            Paragraph::new(line).style(STYLE_SELECTED)
        } else {
            Paragraph::new(line)
        };

        frame.render_widget(paragraph, chunks[4 + idx]);
    }

    let help = Paragraph::new(Line::from(vec![
        Span::styled("[↑/↓]", STYLE_NORMAL),
        Span::styled(" Mode  ", STYLE_MUTED),
        Span::styled("[Tab]", STYLE_NORMAL),
        Span::styled(" Role  ", STYLE_MUTED),
        Span::styled("[Enter]", STYLE_NORMAL),
        Span::styled(" Save  ", STYLE_MUTED),
        Span::styled("[Esc]", STYLE_NORMAL),
        Span::styled(" Cancel", STYLE_MUTED),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(help, chunks[8]);
}
