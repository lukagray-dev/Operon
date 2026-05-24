// Error types for TUI
// Defines all possible error conditions in the TUI layer
// Business logic errors come from operon-rs via AgentBridge

pub mod display;

use std::io;

/// TuiError represents all error conditions that can occur in the TUI layer
#[allow(dead_code)]
/// Business logic errors from operon-rs are wrapped in TuiError::Agent
#[derive(Debug)]
pub enum TuiError {
    /// I/O error (terminal, file system, etc.)
    Io(io::Error),
    
    /// Error from agent backend (operon-rs)
    Agent(anyhow::Error),
    
    /// Rendering error (layout calculation, widget rendering, etc.)
    Render(String),
    
    /// Configuration error (invalid config file, missing required fields, etc.)
    Config(String),
    
    /// Unknown or unexpected error
    Unknown(String),
}

impl std::fmt::Display for TuiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TuiError::Io(e) => write!(f, "I/O error: {}", e),
            TuiError::Agent(e) => write!(f, "Agent error: {}", e),
            TuiError::Render(msg) => write!(f, "Render error: {}", msg),
            TuiError::Config(msg) => write!(f, "Config error: {}", msg),
            TuiError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for TuiError {}

impl From<io::Error> for TuiError {
    fn from(err: io::Error) -> Self {
        TuiError::Io(err)
    }
}

impl From<anyhow::Error> for TuiError {
    fn from(err: anyhow::Error) -> Self {
        TuiError::Agent(err)
    }
}

/// Result type alias for TUI operations
#[allow(dead_code)]
pub type TuiResult<T> = Result<T, TuiError>;
