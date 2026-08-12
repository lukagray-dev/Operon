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
#[derive(Clone)]
pub struct AppState {
    ui_scale: f32,
    reload_generation: i32,
    active_session_id: Option<String>,
    current_project_dir: Option<String>,
    discovered_models: Vec<operon_rs::DiscoveredModel>,
    /// Pending attachments for the next message send, in the order attached.
    pending_attachments: Vec<crate::media::PendingAttachment>,
    /// User preferences loaded from disk.
    prefs: crate::settings::prefs::GuiPrefs,
    /// Optional weak reference to the main application window handle for live property synchronization.
    main_window: Option<slint::Weak<crate::OperonWindow>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("ui_scale", &self.ui_scale)
            .field("reload_generation", &self.reload_generation)
            .field("active_session_id", &self.active_session_id)
            .field("current_project_dir", &self.current_project_dir)
            .field("discovered_models", &self.discovered_models)
            .field("pending_attachments", &self.pending_attachments)
            .field("prefs", &self.prefs)
            .field("main_window", &self.main_window.is_some())
            .finish()
    }
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
            prefs: crate::settings::prefs::GuiPrefs::load(),
            main_window: None,
        }
    }
}

impl AppState {
    /// Registers the active main window instance so state updates can propagate live to the UI.
    pub fn register_main_window(&mut self, window: &crate::OperonWindow) {
        use slint::ComponentHandle;
        self.main_window = Some(window.as_weak());
    }

    /// Updates the auto-scroll-stream preference, saves it to `~/.operon/gui_settings.toml`, and live-syncs the main window.
    pub fn set_auto_scroll_stream(&mut self, enabled: bool) {
        self.prefs.auto_scroll_stream = enabled;
        if let Err(err) = self.prefs.save() {
            tracing::warn!("[operon-gui][state] Failed to save prefs after updating auto_scroll_stream: {err:#}");
        }
        if let Some(weak) = &self.main_window {
            if let Some(window) = weak.upgrade() {
                window.set_auto_scroll_stream(enabled);
            }
        }
    }

    /// Updates the notify-on-permission-request preference and saves it to `~/.operon/gui_settings.toml`.
    pub fn set_notify_on_permission_request(&mut self, enabled: bool) {
        self.prefs.notify_on_permission_request = enabled;
        if let Err(err) = self.prefs.save() {
            tracing::warn!("[operon-gui][state] Failed to save prefs after updating notify_on_permission_request: {err:#}");
        }
    }

    /// Updates the notify-on-response-complete preference and saves it to `~/.operon/gui_settings.toml`.
    pub fn set_notify_on_response_complete(&mut self, enabled: bool) {
        self.prefs.notify_on_response_complete = enabled;
        if let Err(err) = self.prefs.save() {
            tracing::warn!("[operon-gui][state] Failed to save prefs after updating notify_on_response_complete: {err:#}");
        }
    }

    /// Returns a reference to the active user preferences.
    pub fn prefs(&self) -> &crate::settings::prefs::GuiPrefs {
        &self.prefs
    }

    /// Returns a mutable reference to the user preferences.
    pub fn prefs_mut(&mut self) -> &mut crate::settings::prefs::GuiPrefs {
        &mut self.prefs
    }

    /// Sets the active user preferences.
    pub fn set_prefs(&mut self, prefs: crate::settings::prefs::GuiPrefs) {
        self.prefs = prefs;
    }

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
