// Models screen
// Provider list and configuration interface
// For bootstrap: renders placeholder content

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::ui::theme::{STYLE_ACTIVE_BORDER, STYLE_MUTED, STYLE_TITLE};

/// Render the models configuration screen
/// For bootstrap: displays placeholder content
/// Future: Implement provider selection and configuration:
/// - List of providers (OpenAI, Anthropic, local, custom)
/// - API key input fields
/// - Endpoint configuration
/// - Model selection dropdown
/// - Test connection button
pub fn render_models_screen(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(STYLE_ACTIVE_BORDER)
        .title("Model Configuration");

    let lines = vec![
        Line::from(Span::styled("Model Providers", STYLE_TITLE)),
        Line::from(""),
        Line::from(Span::styled("Coming soon:", STYLE_MUTED)),
        Line::from(Span::styled("- OpenAI (GPT-4, GPT-3.5)", STYLE_MUTED)),
        Line::from(Span::styled("- Anthropic (Claude)", STYLE_MUTED)),
        Line::from(Span::styled("- Local models (Ollama, LM Studio)", STYLE_MUTED)),
        Line::from(Span::styled("- Custom OpenAI-compatible APIs", STYLE_MUTED)),
        Line::from(""),
        Line::from(Span::styled("Configure API keys, endpoints, and model selection here.", STYLE_MUTED)),
    ];

    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}
