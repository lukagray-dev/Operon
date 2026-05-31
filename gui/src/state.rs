//! Shared application state for the GUI shell.
//!
//! The UI does not have much live state yet, but the zoom and reload values are
//! already modeled here so later screens can bind to them without changing the
//! titlebar wiring again.

const MIN_UI_SCALE: f32 = 0.8;
const MAX_UI_SCALE: f32 = 1.5;
const UI_SCALE_STEP: f32 = 0.1;
const DEFAULT_UI_SCALE: f32 = 1.0;

/// Small state bundle that tracks the titlebar's user-facing view controls.
#[derive(Debug, Clone)]
pub struct AppState {
    ui_scale: f32,
    reload_generation: i32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ui_scale: DEFAULT_UI_SCALE,
            reload_generation: 0,
        }
    }
}

impl AppState {
    /// Creates a fresh state object with the normal 100% zoom level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current UI scale used by the titlebar and future content.
    pub fn ui_scale(&self) -> f32 {
        self.ui_scale
    }

    /// Returns the current reload generation counter.
    pub fn reload_generation(&self) -> i32 {
        self.reload_generation
    }

    /// Increases the UI scale by one step, clamped to the supported range.
    pub fn zoom_in(&mut self) {
        self.set_ui_scale(self.ui_scale + UI_SCALE_STEP);
    }

    /// Decreases the UI scale by one step, clamped to the supported range.
    pub fn zoom_out(&mut self) {
        self.set_ui_scale(self.ui_scale - UI_SCALE_STEP);
    }

    /// Resets the UI scale back to the default 100% state.
    pub fn reset_zoom(&mut self) {
        self.set_ui_scale(DEFAULT_UI_SCALE);
    }

    /// Updates the UI scale while keeping it inside the supported guard rails.
    pub fn set_ui_scale(&mut self, value: f32) {
        self.ui_scale = value.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
    }

    /// Increments the reload generation and returns the new value.
    pub fn mark_reload(&mut self) -> i32 {
        self.reload_generation = self.reload_generation.saturating_add(1);
        self.reload_generation
    }
}
