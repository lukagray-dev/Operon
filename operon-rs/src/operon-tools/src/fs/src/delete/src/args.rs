//! Argument types for the delete tool.
//!
//! Hey friend! This module defines the defensive deserialization schema for the delete tool's input.
//! The tool accepts a path and an optional permanent flag, supporting common LLM parameter aliases.

use serde::Deserialize;

/// Arguments for the delete tool.
///
/// Specifies a file or directory path and whether to permanently delete it or move it to trash.
/// The path must exist — this tool does not create or delete non-existent paths.
#[derive(Debug, Deserialize)]
pub struct DeleteArgs {
    /// Absolute path to the file or directory to delete.
    /// The path must exist — if it does not, the tool returns an error.
    /// Both files and directories are supported. For directories, the entire tree is deleted.
    #[serde(
        alias = "file_path",
        alias = "filePath",
        alias = "filepath",
        alias = "target_file",
        alias = "targetFile",
        alias = "file",
        alias = "filename",
        alias = "fileName"
    )]
    pub path: String,

    /// If false (default), move the target to the system trash — recoverable
    /// from Trash/Recycle Bin. If true, permanently delete with no recovery
    /// possible. Prefer false unless permanent deletion is explicitly required.
    #[serde(
        default,
        alias = "force",
        alias = "hard_delete",
        alias = "hardDelete",
        alias = "recursive"
    )]
    pub permanent: bool,
}
