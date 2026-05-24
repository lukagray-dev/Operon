// Right panel content types
// Defines what can be displayed in the right sidebar

use std::path::PathBuf;

/// Content types that can be displayed in the right sidebar
/// The right sidebar is hidden when AppState.right_panel is None
#[derive(Debug, Clone)]
pub enum RightPanelContent {
    /// Display a read-only file preview
    /// Shows file content with syntax highlighting (future)
    FilePreview(PathBuf),
    
    /// Display a unified diff
    /// Raw diff string for now, will be parsed and styled later
    #[allow(dead_code)]
    Diff(String),
    
    /// Display an embedded pseudo-terminal
    /// For running commands and viewing output
    Terminal,
}

impl RightPanelContent {
    /// Get a human-readable title for the panel content
    /// Used in the panel header
    #[allow(dead_code)]
    pub fn title(&self) -> String {
        match self {
            RightPanelContent::FilePreview(path) => {
                format!("Preview: {}", path.file_name().unwrap_or_default().to_string_lossy())
            }
            RightPanelContent::Diff(_) => "Diff".to_string(),
            RightPanelContent::Terminal => "Terminal".to_string(),
        }
    }
}
