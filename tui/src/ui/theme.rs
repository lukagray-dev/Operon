// theme.rs — Single source of truth for all colors, styles, and visual design in Operon TUI.
//
// DESIGN & AESTHETICS:
// Operon uses a modern, high-contrast terminal palette tailored for readability:
// - Accent / Brand Blue: #60a5fa (RGB 96, 165, 250)
// - Crisp Foreground:   #f8fafc (RGB 248, 250, 252)
// - Slate Label Text:    #cbd5e1 (RGB 203, 213, 225)
// - Muted / Secondary:   #94a3b8 (RGB 148, 163, 184)
// - Slate Border:        #334155 (RGB 51, 65, 85)
// - Dark Panel Fill:     #1e293b (RGB 30, 41, 59)
//
// ZERO EMOJIS:
// All indicators use clean Unicode box-drawing, braille spinners, or geometric glyphs.

#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

// ============================================================================
// COLOR PALETTE
// ============================================================================

/// Primary background color (terminal default reset).
pub const COLOR_BG: Color = Color::Reset;

/// Primary foreground / text color (#f8fafc).
pub const COLOR_FG: Color = Color::Rgb(248, 250, 252);

/// Accent blue for highlights, focus borders, and active indicators (#60a5fa).
pub const COLOR_ACCENT: Color = Color::Rgb(96, 165, 250);

/// Secondary light blue accent (#93c5fd).
pub const COLOR_ACCENT_2: Color = Color::Rgb(147, 197, 253);

/// Slate text color for secondary labels (#cbd5e1).
pub const COLOR_LABEL: Color = Color::Rgb(203, 213, 225);

/// Success / positive state color (#4ade80).
pub const COLOR_SUCCESS: Color = Color::Rgb(74, 222, 128);

/// Warning / attention state color (#fbbf24).
pub const COLOR_WARNING: Color = Color::Rgb(251, 191, 36);

/// Error / danger state color (#f87171).
pub const COLOR_ERROR: Color = Color::Rgb(248, 113, 113);

/// Muted / dimmed text color (#94a3b8).
pub const COLOR_MUTED: Color = Color::Rgb(148, 163, 184);

/// Border color for inactive panels (#334155).
pub const COLOR_BORDER_INACTIVE: Color = Color::Rgb(51, 65, 85);

/// Border color for active / focused panels (#60a5fa).
pub const COLOR_BORDER_ACTIVE: Color = Color::Rgb(96, 165, 250);

/// Agent message color in chat (#f8fafc).
pub const COLOR_AGENT: Color = Color::Rgb(248, 250, 252);

/// User message color in chat (#60a5fa).
pub const COLOR_USER: Color = Color::Rgb(96, 165, 250);

// ============================================================================
// REUSABLE STYLE DEFINITIONS
// ============================================================================

/// Default body text style.
pub const STYLE_NORMAL: Style = Style::new().fg(COLOR_FG).bg(COLOR_BG);

/// Selected / highlighted item style in lists and menus.
pub const STYLE_SELECTED: Style = Style::new()
    .fg(Color::Rgb(15, 23, 42)) // Deep slate-900 background text
    .bg(COLOR_ACCENT)
    .add_modifier(Modifier::BOLD);

/// Active / focused text style.
pub const STYLE_ACTIVE: Style = Style::new().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD);

/// Inactive / unfocused element style.
pub const STYLE_INACTIVE: Style = Style::new().fg(COLOR_MUTED);

/// Border style for active / focused panels.
pub const STYLE_ACTIVE_BORDER: Style = Style::new().fg(COLOR_BORDER_ACTIVE);

/// Border style for inactive panels.
pub const STYLE_INACTIVE_BORDER: Style = Style::new().fg(COLOR_BORDER_INACTIVE);

/// Status bar base style.
pub const STYLE_STATUSBAR: Style = Style::new().fg(COLOR_FG).bg(COLOR_BG);

/// Error message style.
pub const STYLE_ERROR: Style = Style::new().fg(COLOR_ERROR).add_modifier(Modifier::BOLD);

/// Warning message style.
pub const STYLE_WARNING: Style = Style::new().fg(COLOR_WARNING).add_modifier(Modifier::BOLD);

/// Success message style.
pub const STYLE_SUCCESS: Style = Style::new().fg(COLOR_SUCCESS).add_modifier(Modifier::BOLD);

/// Agent message style in chat.
pub const STYLE_AGENT_MSG: Style = Style::new().fg(COLOR_AGENT);

/// User message style in chat.
pub const STYLE_USER_MSG: Style = Style::new().fg(COLOR_USER);

/// Muted / dimmed text style.
pub const STYLE_MUTED: Style = Style::new().fg(COLOR_MUTED);

/// Secondary label text style.
pub const STYLE_LABEL: Style = Style::new().fg(COLOR_LABEL);

/// Title / header style.
pub const STYLE_TITLE: Style = Style::new().fg(COLOR_ACCENT).add_modifier(Modifier::BOLD);

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Creates a styled span with the given text and style.
pub fn styled_text<'a>(text: &'a str, style: Style) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(text, style)
}

/// Creates a bold styled span.
pub fn bold<'a>(text: &'a str) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(text, Style::default().add_modifier(Modifier::BOLD))
}

/// Creates a dimmed / muted span.
pub fn muted<'a>(text: &'a str) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(text, STYLE_MUTED)
}
