// SessionContext struct
// Read-only data passed down from operon-rs backend
// Contains runtime information about the current agent session
// TUI displays this data but never modifies it

/// Session context provided by operon-rs backend
/// This is read-only data that the TUI displays in the status bar and other UI elements
/// All fields have sensible defaults for when the backend is not yet connected
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// Name of the currently active model (e.g., "claude-sonnet-4.5", "gpt-4")
    pub model_name: String,
    
    /// Current context window usage in tokens
    pub context_used: usize,
    
    /// Maximum context window size in tokens
    pub context_max: usize,
    
    /// Current agent status (e.g., "Idle", "Thinking", "Executing", "Error")
    #[allow(dead_code)]
    pub agent_status: String,
    
    /// Whether the agent is currently processing a request
    #[allow(dead_code)]
    pub is_busy: bool,
}

impl Default for SessionContext {
    /// Create a SessionContext with placeholder values
    /// Used when the backend is not yet connected or during initialization
    fn default() -> Self {
        Self {
            model_name: "No model selected".to_string(),
            context_used: 0,
            context_max: 200_000,
            agent_status: "Idle".to_string(),
            is_busy: false,
        }
    }
}

impl SessionContext {
    /// Calculate context usage as a percentage (0-100)
    /// Used for rendering progress bars in the status bar
    pub fn context_percentage(&self) -> u8 {
        if self.context_max == 0 {
            return 0;
        }
        let percentage = (self.context_used as f64 / self.context_max as f64 * 100.0) as u8;
        percentage.min(100)
    }

    /// Format context usage as a human-readable string
    /// Example: "45.2K / 200K (22%)"
    pub fn context_display(&self) -> String {
        let used_k = self.context_used as f64 / 1000.0;
        let max_k = self.context_max as f64 / 1000.0;
        let percentage = self.context_percentage();
        format!("{:.1}K / {:.0}K ({}%)", used_k, max_k, percentage)
    }
}
