//! Shared module for Operon GUI backend.

pub mod dwm;
pub mod state;

pub use dwm::apply_window_dwm_styling;
pub use state::AppState;
