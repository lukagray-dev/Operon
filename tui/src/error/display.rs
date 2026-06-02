// Human-readable error formatting
// Converts TuiError into user-friendly messages for display in status bar
// Strips technical details and provides actionable information

use super::TuiError;

#[allow(dead_code)]
impl TuiError {
    /// Format error for display in status bar
    /// Keeps messages concise and user-friendly
    /// Technical details are logged but not shown to user
    pub fn display_message(&self) -> String {
        match self {
            TuiError::Io(e) => format!("I/O error: {}", e),
            TuiError::Agent(e) => format!("Agent error: {}", e),
            TuiError::Render(msg) => format!("Render error: {}", msg),
            TuiError::Config(msg) => format!("Config error: {}", msg),
            TuiError::Unknown(msg) => format!("Error: {}", msg),
        }
    }

    /// Get error severity for styling
    /// Used to determine color/style in status bar
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            TuiError::Io(_) => ErrorSeverity::Critical,
            TuiError::Agent(_) => ErrorSeverity::Warning,
            TuiError::Render(_) => ErrorSeverity::Warning,
            TuiError::Config(_) => ErrorSeverity::Error,
            TuiError::Unknown(_) => ErrorSeverity::Error,
        }
    }
}

/// Error severity levels for UI styling
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Informational message (blue)
    Info,

    /// Warning that doesn't prevent operation (yellow)
    Warning,

    /// Error that prevents current operation (orange)
    Error,

    /// Critical error that may require restart (red)
    Critical,
}
