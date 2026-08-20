// session.rs — SessionContext holding runtime information about active model and context tokens.
//
// ZERO BUSINESS LOGIC IN FRONTEND:
// The TUI queries `operon-rs` backend (`operon_config::load()`, `operon_diff::current_branch_workspace()`)
// for real model metadata, token capacity, and git branch, displaying it in the status bar and header bars.

/// Formats a token count into a clean, human-readable representation (e.g. "128K", "1.0M").
pub fn format_tokens(tokens: usize) -> String {
    if tokens >= 1_000_000 {
        let m = tokens as f64 / 1_000_000.0;
        format!("{:.1}M", m)
    } else if tokens >= 1_000 {
        let k = tokens as f64 / 1_000.0;
        if tokens % 1_000 == 0 {
            format!("{:.0}K", k)
        } else {
            format!("{:.1}K", k)
        }
    } else {
        format!("{}", tokens)
    }
}

/// Session context provided by operon-rs backend.
///
/// This is runtime state that the TUI displays in the status bar and other UI elements.
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// Name of the currently active model (e.g., "claude-3-5-sonnet-latest", "gpt-4o", "gemini-2.5-flash").
    pub model_name: String,

    /// Current context window usage in tokens.
    pub context_used: usize,

    /// Maximum context window capacity in tokens (queried from operon-rs config / model discovery).
    pub context_max: usize,

    /// Currently checked-out Git branch (queried via operon_rs::diff).
    pub git_branch: String,

    /// TUI-local auto-approve toggle indicator.
    pub auto_approve: bool,

    /// Current agent status (e.g., "Idle", "Thinking", "Executing", "Error").
    #[allow(dead_code)]
    pub agent_status: String,

    /// Whether the agent is currently processing a request.
    #[allow(dead_code)]
    pub is_busy: bool,
}

impl Default for SessionContext {
    /// Create a SessionContext with values initialized from operon_rs::load().
    fn default() -> Self {
        Self::load_from_backend()
    }
}

impl SessionContext {
    /// Loads session context from the active operon-rs config and repository diff engine.
    pub fn load_from_backend() -> Self {
        let (model_name, context_max) = match operon_rs::load() {
            Ok(config) => {
                let model_id = config.provider.model.model_id.clone();
                let context_window = config.provider.model.context_window;
                if model_id.trim().is_empty() {
                    ("Not configured".to_string(), 0)
                } else {
                    (model_id, context_window)
                }
            }
            Err(_) => ("Not configured".to_string(), 0),
        };

        let git_branch = operon_rs::diff::current_branch_workspace(".")
            .map(|b| b.name)
            .unwrap_or_else(|_| "-".to_string());

        Self {
            model_name,
            context_used: 0,
            context_max,
            git_branch,
            auto_approve: false,
            agent_status: "Idle".to_string(),
            is_busy: false,
        }
    }

    /// Refreshes model name, context window capacity, and Git branch from backend config.
    pub fn refresh_from_backend(&mut self) {
        if let Ok(config) = operon_rs::load() {
            let model_id = config.provider.model.model_id.clone();
            if model_id.trim().is_empty() {
                self.model_name = "Not configured".to_string();
                self.context_max = 0;
            } else {
                self.model_name = model_id;
                self.context_max = config.provider.model.context_window;
            }
        }

        if let Ok(branch) = operon_rs::diff::current_branch_workspace(".") {
            self.git_branch = branch.name;
        }
    }

    /// Calculate context usage as a percentage (0-100).
    pub fn context_percentage(&self) -> u8 {
        if self.context_max == 0 {
            return 0;
        }
        let percentage = (self.context_used as f64 / self.context_max as f64 * 100.0) as u8;
        percentage.min(100)
    }

    /// Format context usage as a clean, human-readable string.
    /// Example: "0 / 128K (0%)" or "12.5K / 200K (6%)" or "0 / 1.0M (0%)", or "-" when not configured.
    pub fn context_display(&self) -> String {
        if self.context_max == 0 || self.model_name == "Not configured" {
            return "-".to_string();
        }

        let used_str = format_tokens(self.context_used);
        let max_str = format_tokens(self.context_max);
        let percentage = self.context_percentage();
        format!("{} / {} ({}%)", used_str, max_str, percentage)
    }
}
