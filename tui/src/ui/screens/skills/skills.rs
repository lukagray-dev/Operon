// Skills screen
// Skill manager with enable/disable toggles and OHub integration
// For bootstrap: renders placeholder content

use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_TITLE};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Render the skills management screen
/// For bootstrap: displays placeholder content
/// Future: Implement skill manager:
/// - List of installed skills with enable/disable toggles
/// - Skill descriptions and metadata
/// - Browse OHub marketplace
/// - Install/uninstall skills
/// - Skill configuration options
pub fn render_skills_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Skills");

    let lines = vec![
        Line::from(Span::styled("Skill Manager", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Coming soon:", STYLE_MUTED)),
        Line::from(Span::styled("- List of installed skills", STYLE_MUTED)),
        Line::from(Span::styled("- Enable/disable toggles", STYLE_MUTED)),
        Line::from(Span::styled("- Browse OHub marketplace", STYLE_MUTED)),
        Line::from(Span::styled("- Install new skills", STYLE_MUTED)),
        Line::from(Span::styled("- Skill configuration", STYLE_MUTED)),
        Line::from(""),
        Line::from(Span::styled(
            "Extend Operon's capabilities with community skills.",
            STYLE_MUTED,
        )),
    ];

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}
