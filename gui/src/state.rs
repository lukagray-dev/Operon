//! Shared application state for the GUI shell.
//!
//! The UI does not have much live state yet, but the zoom and reload values are
//! already modeled here so later screens can bind to them without changing the
//! titlebar wiring again.

const MIN_UI_SCALE: f32 = 0.8;
const MAX_UI_SCALE: f32 = 1.5;
const UI_SCALE_STEP: f32 = 0.1;
const DEFAULT_UI_SCALE: f32 = 1.0;

/// Small state bundle that tracks the titlebar's user-facing view controls and active session state.
#[derive(Debug, Clone)]
pub struct AppState {
    ui_scale: f32,
    reload_generation: i32,
    active_session_id: Option<String>,
    current_project_dir: Option<String>,
    discovered_models: Vec<operon_rs::DiscoveredModel>,
    /// Pending attachments for the next message send, in the order attached.
    pending_attachments: Vec<crate::media::PendingAttachment>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            ui_scale: DEFAULT_UI_SCALE,
            reload_generation: 0,
            active_session_id: None,
            current_project_dir: None,
            discovered_models: Vec::new(),
            pending_attachments: Vec::new(),
        }
    }
}

impl AppState {
    /// Creates a fresh state object with the normal 100% zoom level.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current active session ID, if any.
    pub fn active_session_id(&self) -> Option<&str> {
        self.active_session_id.as_deref()
    }

    /// Sets the active session ID.
    pub fn set_active_session_id(&mut self, id: Option<String>) {
        self.active_session_id = id;
    }

    /// Returns the current project directory path, if any.
    pub fn current_project_dir(&self) -> Option<&str> {
        self.current_project_dir.as_deref()
    }

    /// Sets the current project directory path.
    pub fn set_current_project_dir(&mut self, dir: Option<String>) {
        self.current_project_dir = dir;
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

    /// Returns the slice of discovered models from live discovery.
    pub fn discovered_models(&self) -> &[operon_rs::DiscoveredModel] {
        &self.discovered_models
    }

    /// Sets the list of discovered models.
    pub fn set_discovered_models(&mut self, models: Vec<operon_rs::DiscoveredModel>) {
        self.discovered_models = models;
    }

    /// Returns a slice of current pending attachments.
    pub fn pending_attachments(&self) -> &[crate::media::PendingAttachment] {
        &self.pending_attachments
    }

    /// Adds a new attachment to the pending list.
    pub fn add_attachment(&mut self, attachment: crate::media::PendingAttachment) {
        self.pending_attachments.push(attachment);
    }

    /// Removes a pending attachment by index.
    pub fn remove_attachment(&mut self, index: usize) {
        if index < self.pending_attachments.len() {
            self.pending_attachments.remove(index);
        }
    }

    /// Clears all pending attachments.
    pub fn clear_attachments(&mut self) {
        self.pending_attachments.clear();
    }
}
