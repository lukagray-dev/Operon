// Theme and style constants
// Single source of truth for all colors, styles, and visual design
// Change the color palette here to restyle the entire TUI

// Allow dead code for theme constants that will be used as the UI is built out
#![allow(dead_code)]

use ratatui::style::{Color, Modifier, Style};

// ============================================================================
// COLOR PALETTE
// All colors used in the TUI are defined here
// Modify these constants to change the entire color scheme
// ============================================================================

// Allow dead code for theme constants that will be used as the UI is built out
#[allow(dead_code)]

/// Primary background color
pub const COLOR_BG: Color = Color::Reset;

/// Primary foreground/text color
pub const COLOR_FG: Color = Color::Reset;

/// Accent color for highlights and active elements
pub const COLOR_ACCENT: Color = Color::Cyan;

/// Secondary accent color
pub const COLOR_ACCENT_2: Color = Color::Blue;

/// Success/positive state color
pub const COLOR_SUCCESS: Color = Color::Green;

/// Warning state color
pub const COLOR_WARNING: Color = Color::Yellow;

/// Error/danger state color
pub const COLOR_ERROR: Color = Color::Red;

/// Muted/dimmed text color
pub const COLOR_MUTED: Color = Color::DarkGray;

/// Border color for inactive elements
pub const COLOR_BORDER_INACTIVE: Color = Color::DarkGray;

/// Border color for active/focused elements
pub const COLOR_BORDER_ACTIVE: Color = Color::Cyan;

/// Agent message color in chat
pub const COLOR_AGENT: Color = Color::Cyan;

/// User message color in chat
pub const COLOR_USER: Color = Color::Green;

// ============================================================================
// STYLE CONSTANTS
// Reusable style definitions for common UI elements
// ============================================================================

/// Default text style
pub const STYLE_NORMAL: Style = Style::new().fg(COLOR_FG).bg(COLOR_BG);

/// Selected/highlighted item style
pub const STYLE_SELECTED: Style = Style::new()
    .fg(COLOR_BG)
    .bg(COLOR_ACCENT)
    .add_modifier(Modifier::BOLD);

/// Active/focused element style
pub const STYLE_ACTIVE: Style = Style::new()
    .fg(COLOR_ACCENT)
    .add_modifier(Modifier::BOLD);

/// Inactive/unfocused element style
pub const STYLE_INACTIVE: Style = Style::new().fg(COLOR_MUTED);

/// Border style for active/focused panels
pub const STYLE_ACTIVE_BORDER: Style = Style::new().fg(COLOR_BORDER_ACTIVE);

/// Border style for inactive panels
pub const STYLE_INACTIVE_BORDER: Style = Style::new().fg(COLOR_BORDER_INACTIVE);

/// Status bar style
pub const STYLE_STATUSBAR: Style = Style::new()
    .fg(COLOR_BG)
    .bg(COLOR_ACCENT)
    .add_modifier(Modifier::BOLD);

/// Error message style
pub const STYLE_ERROR: Style = Style::new()
    .fg(COLOR_ERROR)
    .add_modifier(Modifier::BOLD);

/// Warning message style
pub const STYLE_WARNING: Style = Style::new()
    .fg(COLOR_WARNING)
    .add_modifier(Modifier::BOLD);

/// Success message style
pub const STYLE_SUCCESS: Style = Style::new()
    .fg(COLOR_SUCCESS)
    .add_modifier(Modifier::BOLD);

/// Agent message style in chat
pub const STYLE_AGENT_MSG: Style = Style::new().fg(COLOR_AGENT);

/// User message style in chat
pub const STYLE_USER_MSG: Style = Style::new().fg(COLOR_USER);

/// Muted/dimmed text style
pub const STYLE_MUTED: Style = Style::new().fg(COLOR_MUTED);

/// Title/header style
pub const STYLE_TITLE: Style = Style::new()
    .fg(COLOR_ACCENT)
    .add_modifier(Modifier::BOLD);

// ============================================================================
// HELPER FUNCTIONS
// Utility functions for creating styled text
// ============================================================================

/// Create a styled span with the given text and style
pub fn styled_text<'a>(text: &'a str, style: Style) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(text, style)
}

/// Create a bold span
pub fn bold<'a>(text: &'a str) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(text, Style::default().add_modifier(Modifier::BOLD))
}

/// Create a dimmed/muted span
pub fn muted<'a>(text: &'a str) -> ratatui::text::Span<'a> {
    ratatui::text::Span::styled(text, STYLE_MUTED)
}
