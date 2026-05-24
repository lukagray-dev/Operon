// File system state for left sidebar
// Tracks cursor position, expanded directories, and selected file
// For bootstrap: empty stub, will be implemented when file tree is added

use std::path::PathBuf;

/// State for the file tree in the left sidebar
#[allow(dead_code)]
/// Tracks which directories are expanded, which file is selected, etc.
/// For bootstrap: minimal stub implementation
#[derive(Debug, Clone)]
pub struct FileSystemState {
    /// Currently selected file/directory path
    pub selected_path: Option<PathBuf>,
    
    /// List of expanded directory paths
    /// Directories in this set show their children in the tree
    pub expanded_dirs: Vec<PathBuf>,
    
    /// Cursor position in the file tree (row index)
    pub cursor: usize,
}

#[allow(dead_code)]
impl FileSystemState {
    /// Create a new FileSystemState with no selection
    pub fn new() -> Self {
        Self {
            selected_path: None,
            expanded_dirs: Vec::new(),
            cursor: 0,
        }
    }

    /// Check if a directory is expanded
    pub fn is_expanded(&self, path: &PathBuf) -> bool {
        self.expanded_dirs.contains(path)
    }

    /// Toggle expansion state of a directory
    pub fn toggle_expand(&mut self, path: PathBuf) {
        if let Some(pos) = self.expanded_dirs.iter().position(|p| p == &path) {
            self.expanded_dirs.remove(pos);
        } else {
            self.expanded_dirs.push(path);
        }
    }

    /// Set the selected path
    pub fn select(&mut self, path: Option<PathBuf>) {
        self.selected_path = path;
    }
}

impl Default for FileSystemState {
    fn default() -> Self {
        Self::new()
    }
}
