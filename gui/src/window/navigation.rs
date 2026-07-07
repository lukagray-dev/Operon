//! Placeholder navigation logic for the titlebar's back and forward buttons.
//!
//! The buttons are intentionally visual-only in this first pass, but keeping a
//! small Rust type here makes the future history/navigation plumbing obvious.

/// Represents one of the titlebar's browser-like navigation directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationDirection {
    Back,
    Forward,
}

/// Returns the human-readable label for the direction.
pub fn label(direction: NavigationDirection) -> &'static str {
    match direction {
        NavigationDirection::Back => "Back",
        NavigationDirection::Forward => "Forward",
    }
}
