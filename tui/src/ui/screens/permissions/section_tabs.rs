// Section tabs component
// Renders the "Global • Directory" tab switcher
// Active tab is highlighted with accent color, inactive is muted

use crate::ui::screens::permissions::state::PermissionsSection;
use crate::ui::theme::{COLOR_ACCENT, STYLE_MUTED};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// Build the section tabs title line for the permissions screen
/// Returns a Line with styled spans: "Global • Directory"
/// Active section is highlighted with accent color + bold
/// Inactive section is muted gray
/// Separator bullet is always muted
pub fn build_section_tabs_title(active_section: PermissionsSection) -> Line<'static> {
    let (global_style, directory_style) = match active_section {
        PermissionsSection::Global => (
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
            STYLE_MUTED,
        ),
        PermissionsSection::Directory => (
            STYLE_MUTED,
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
    };

    Line::from(vec![
        Span::styled("Global", global_style),
        Span::styled(" • ", STYLE_MUTED),
        Span::styled("Directory", directory_style),
    ])
}
