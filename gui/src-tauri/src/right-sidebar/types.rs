//! Data Transfer Objects for the Source Control (Git Diff & Graph) Right Sidebar.

use serde::{Deserialize, Serialize};

/// Line-by-line diff entry inside a diff hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffLineDto {
    /// Line type: '+' for addition, '-' for deletion, ' ' for context
    pub line_type: String,
    /// Raw line text content
    pub content: String,
    /// Line number in the original/base file
    pub old_line_num: String,
    /// Line number in the modified/new file
    pub new_line_num: String,
}

/// Hunk of code modifications in a file diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffHunkDto {
    /// Unified diff header text (e.g. "@@ -10,4 +10,6 @@")
    pub header: String,
    /// List of lines within this hunk
    pub lines: Vec<GitDiffLineDto>,
}

/// Single file modification entry matching Slint `GitFileDiff`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFileDiffDto {
    /// Full relative file path (e.g. "gui/src/ts/right-sidebar/right-sidebar.ts")
    pub path: String,
    /// Basename of the file (e.g. "right-sidebar.ts")
    pub file_name: String,
    /// Directory path prefix (e.g. "gui/src/ts/right-sidebar")
    pub dir_path: String,
    /// Status: "modified", "added", "deleted", "untracked", "renamed"
    pub status: String,
    /// Inserted lines count
    pub insertions: i32,
    /// Deleted lines count
    pub deletions: i32,
    /// Diff hunks for line-by-line preview
    pub hunks: Vec<GitDiffHunkDto>,
    /// Whether this file's diff viewer is expanded
    pub is_expanded: bool,
}

/// Information about a git repository entry in the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitRepositoryInfoDto {
    /// Repository name (e.g. "Operon")
    pub name: String,
    /// Active branch name (e.g. "main", "feature/auth")
    pub branch: String,
    /// Whether this is the active repo
    pub is_active: bool,
    /// Whether this repo has uncommitted changes
    pub has_changes: bool,
}

/// Visual Git Commit Graph node entry matching Slint `GitGraphCommit`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitGraphCommitDto {
    /// Full commit SHA hash
    pub hash: String,
    /// Short 7-char commit hash
    pub short_hash: String,
    /// First line summary of the commit message
    pub message: String,
    /// Commit author name or email
    pub author: String,
    /// Branch tag badge label if branch tip/HEAD (e.g. "main", "v1.0")
    pub branch_tag: String,
    /// Whether this commit is current repository HEAD
    pub is_head: bool,
    /// Whether this commit is local-only (unpushed)
    pub is_local: bool,
}

/// Full source control diff details for the active workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffDetailsDto {
    /// True if a valid Git repository exists in the workspace
    pub has_repo: bool,
    /// Repository root folder name
    pub repo_name: String,
    /// Current checked out branch name
    pub current_branch: String,
    /// Total line insertions across all changed files
    pub total_insertions: i32,
    /// Total line deletions across all changed files
    pub total_deletions: i32,
    /// List of unstaged (working directory) changed files
    pub unstaged_files: Vec<GitFileDiffDto>,
    /// List of staged (index) changed files
    pub staged_files: Vec<GitFileDiffDto>,
}

/// Branch metadata DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranchInfoDto {
    pub name: String,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}
