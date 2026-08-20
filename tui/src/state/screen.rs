// ActiveScreen enum
// Defines all possible main-panel screens in the TUI
// Each screen is a full-page view that replaces the main content area

use std::fmt;

/// All possible screens in the Operon TUI
/// The active screen determines what is rendered in the main panel
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    /// Chat interface — message history + input box
    Chat,

    /// Resume previous conversations from workspace history
    Resume,

    /// Model provider configuration — OpenAI, Anthropic, local, custom
    Models,

    /// Permission rules — Owner vs External access control
    Permissions,

    /// Skills manager — enable/disable/download from OHub
    Skills,

    /// Extensions manager — install/remove/configure extensions
    Extensions,

    /// Help screen — keybind reference, searchable
    Help,
}

impl fmt::Display for ActiveScreen {
    /// Format screen name for display in status bar
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActiveScreen::Chat => write!(f, "Chat"),
            ActiveScreen::Resume => write!(f, "Resume"),
            ActiveScreen::Models => write!(f, "Models"),
            ActiveScreen::Permissions => write!(f, "Permissions"),
            ActiveScreen::Skills => write!(f, "Skills"),
            ActiveScreen::Extensions => write!(f, "Extensions"),
            ActiveScreen::Help => write!(f, "Help"),
        }
    }
}

impl ActiveScreen {
    /// Get all screens in order for navigation
    /// Used for tab cycling and screen selection
    pub fn all() -> &'static [ActiveScreen] {
        &[
            ActiveScreen::Chat,
            ActiveScreen::Resume,
            ActiveScreen::Models,
            ActiveScreen::Permissions,
            ActiveScreen::Skills,
            ActiveScreen::Extensions,
            ActiveScreen::Help,
        ]
    }

    /// Get the next screen in the list (wraps around)
    #[allow(dead_code)]
    pub fn next(&self) -> ActiveScreen {
        let all = Self::all();
        let current_idx = all.iter().position(|s| s == self).unwrap_or(0);
        let next_idx = (current_idx + 1) % all.len();
        all[next_idx]
    }

    /// Get the previous screen in the list (wraps around)
    #[allow(dead_code)]
    pub fn prev(&self) -> ActiveScreen {
        let all = Self::all();
        let current_idx = all.iter().position(|s| s == self).unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            all.len() - 1
        } else {
            current_idx - 1
        };
        all[prev_idx]
    }
}
