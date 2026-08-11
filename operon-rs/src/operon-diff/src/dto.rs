// dto.rs — Serde Data Transfer Objects for operon-diff
//
// Hey buddy! This module contains all serde data transfer objects (DTOs) used to transport
// Git state, diff stats, commit histories, branch metadata, and repository status between the
// Rust backend engine and caller frontends (such as the Slint desktop UI).
//
// Every struct uses `#[serde(rename_all = "camelCase")]` to seamlessly translate snake_case Rust
// properties into standard camelCase properties required by Slint and web/IPC serialization.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Represents line-by-line changes in a file patch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    /// Line type indicator: '+' for addition, '-' for deletion, ' ' for context
    pub line_type: char,
    /// The raw text content of the line
    pub content: String,
    /// The line number in the original/old file (None if it is a new line addition)
    pub old_line_num: Option<u32>,
    /// The line number in the modified/new file (None if it was deleted)
    pub new_line_num: Option<u32>,
}

/// Represents a single hunk of modifications (a collection of modified/context lines).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    /// Unified diff header text, e.g. "@@ -245,6 +245,21 @@"
    pub header: String,
    /// Sequential list of lines inside this hunk
    pub lines: Vec<DiffLine>,
    /// Index of the first modified line in the old file
    pub old_start: u32,
    /// Number of old lines modified or removed
    pub old_lines: u32,
    /// Index of the first modified line in the new file
    pub new_start: u32,
    /// Number of new lines added or modified
    pub new_lines: u32,
}

/// Represents diff details for a specific modified or untracked file,
/// matching Slint `GitFileDiff` struct requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    /// Full relative file path (e.g. "gui/ui/right-sidebar/right-sidebar.slint")
    pub path: String,
    /// Basename of the file (e.g. "right-sidebar.slint")
    pub file_name: String,
    /// Directory path prefix (e.g. "gui/ui/right-sidebar")
    pub dir_path: String,
    /// Git file status label, e.g. "modified", "added", "deleted", "untracked", "renamed"
    pub status: String,
    /// Count of inserted lines (+X)
    pub insertions: usize,
    /// Count of deleted lines (-Y)
    pub deletions: usize,
    /// List of modification hunks inside the file diff
    pub hunks: Vec<DiffHunk>,
    /// UI expansion flag for tree/accordion views
    pub is_expanded: bool,
}

/// Top-level DTO representing all modifications in the repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryDiff {
    /// True when a git repository is present in the workspace
    pub has_repo: bool,
    /// Repository root folder name (e.g. "Operon")
    pub repo_name: String,
    /// Total insertions across all files
    pub total_insertions: usize,
    /// Total deletions across all files
    pub total_deletions: usize,
    /// List of files with unstaged/workdir changes
    pub unstaged_files: Vec<FileDiff>,
    /// List of files with staged/index changes
    pub staged_files: Vec<FileDiff>,
}

/// Basic statistics for top-right quick changes count view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffStats {
    /// True if a valid Git repository exists in the workspace
    pub has_repo: bool,
    /// Total line insertions across staged and unstaged changes
    pub insertions: usize,
    /// Total line deletions across staged and unstaged changes
    pub deletions: usize,
}

/// Information about a single repository entry managed by the workspace `RepoRegistry`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    /// Absolute path to the repository root directory
    pub root: PathBuf,
    /// Display name of the repository (usually directory name)
    pub name: String,
    /// Whether this is the currently active repository in the workspace
    pub is_active: bool,
    /// Whether this repository has any staged or unstaged modifications
    pub has_changes: bool,
}

/// Detailed metadata about a Git branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    /// Name of the branch (e.g. "main", "feature/auth")
    pub name: String,
    /// Whether this branch is currently checked out as HEAD
    pub is_head: bool,
    /// Upstream tracking branch name, if configured (e.g. "origin/main")
    pub upstream: Option<String>,
    /// Number of commits this local branch is ahead of its upstream
    pub ahead: usize,
    /// Number of commits this local branch is behind its upstream
    pub behind: usize,
}

/// Data structure representing a single commit node in the visual Git Commit Graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphCommit {
    /// Full 40-character SHA commit hash
    pub hash: String,
    /// Short 7-character commit hash string (e.g. "a1b2c3d")
    pub short_hash: String,
    /// First line summary of the commit message
    pub message: String,
    /// Author name/email associated with the commit
    pub author: String,
    /// Branch tag badge label if this commit is a branch tip (e.g. "main", "v1.0")
    pub branch_tag: String,
    /// True if this commit is the current repository HEAD
    pub is_head: bool,
    /// True if commit is local-only (unpushed to remote tracking branch)
    pub is_local: bool,
}

/// Result returned upon creating or amending a commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    /// OID of the newly created commit string
    pub oid: String,
}
