// lib.rs — Git Diff Engine for Operon
//
// Hey friend! This crate implements the core git integration for Operon.
// It uses the high-performance `git2` library to locate the repository root,
// query unstaged and staged files, compile unified line-by-line diff views,
// and perform index operations like staging, unstaging, and reverting.
//
// Every function is fully documented with detailed inline comments to help
// you understand how libgit2 features are utilized.

use std::path::Path;
use git2::{Diff, DiffOptions, Repository, StatusOptions};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Custom Error type representing any failures encountered during Git operations.
#[derive(Debug, Error)]
pub enum DiffError {
    #[error("Git libgit2 error: {0}")]
    Git(#[from] git2::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No git repository found in workspace hierarchy: {0}")]
    NoRepository(String),

    #[error("HEAD commit resolution failed: {0}")]
    HeadResolution(String),
}

/// Represents line-by-line changes in a file patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Represents diff details for a specific modified or untracked file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    /// Relative path of the file from the repository root
    pub path: String,
    /// Git file status, e.g. "modified", "added", "deleted", "untracked", "renamed"
    pub status: String,
    /// Count of inserted lines
    pub insertions: usize,
    /// Count of deleted lines
    pub deletions: usize,
    /// List of modification hunks inside the file diff
    pub hunks: Vec<DiffHunk>,
}

/// Top-level DTO representing all modifications in the repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffStats {
    pub has_repo: bool,
    pub insertions: usize,
    pub deletions: usize,
}

/// Helper: Discovers the repository root from the given workspace folder path.
///
/// Hey buddy! If the folder passed doesn't have a `.git` folder directly inside it,
/// libgit2 will search parent directories until it finds the repository root.
pub fn discover_repository<P: AsRef<Path>>(workspace_root: P) -> Result<Repository, DiffError> {
    let repo = Repository::discover(workspace_root)
        .map_err(|e| DiffError::NoRepository(e.to_string()))?;
    Ok(repo)
}

/// Resolves stats (insertions/deletions) for the quick changes badge in the header.
pub fn get_diff_stats<P: AsRef<Path>>(workspace_root: P) -> Result<GitDiffStats, DiffError> {
    // 1. Try to open the repository. If not found, return gracefully
    let repo = match discover_repository(workspace_root) {
        Ok(r) => r,
        Err(_) => {
            return Ok(GitDiffStats {
                has_repo: false,
                insertions: 0,
                deletions: 0,
            });
        }
    };

    // 2. Fetch staged stats (Index vs HEAD tree)
    let (staged_ins, staged_del) = match repo.head() {
        Ok(head_ref) => {
            if let Ok(head_tree) = head_ref.peel_to_tree() {
                let diff = repo.diff_tree_to_index(Some(&head_tree), None, None)?;
                let stats = diff.stats()?;
                (stats.insertions(), stats.deletions())
            } else {
                (0, 0)
            }
        }
        Err(_) => (0, 0), // Unborn/new repository
    };

    // 3. Fetch unstaged stats (Workdir vs Index). We include untracked files!
    let mut opts = DiffOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);

    let workdir_diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;
    let workdir_stats = workdir_diff.stats()?;

    let total_ins = staged_ins.saturating_add(workdir_stats.insertions());
    let total_del = staged_del.saturating_add(workdir_stats.deletions());

    Ok(GitDiffStats {
        has_repo: true,
        insertions: total_ins,
        deletions: total_del,
    })
}

/// Query full repository changes detailed tree.
pub fn get_diff_details<P: AsRef<Path>>(workspace_root: P) -> Result<RepositoryDiff, DiffError> {
    // 1. Open the repository
    let repo = match discover_repository(workspace_root) {
        Ok(r) => r,
        Err(_) => {
            return Ok(RepositoryDiff {
                has_repo: false,
                repo_name: String::new(),
                total_insertions: 0,
                total_deletions: 0,
                unstaged_files: Vec::new(),
                staged_files: Vec::new(),
            });
        }
    };

    // Extract repository directory name
    let repo_name = repo.workdir()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();

    // 2. Fetch unstaged changes (Index vs Workdir)
    let mut unstaged_opts = DiffOptions::new();
    unstaged_opts.include_untracked(true);
    unstaged_opts.recurse_untracked_dirs(true);
    let unstaged_diff = repo.diff_index_to_workdir(None, Some(&mut unstaged_opts))?;
    let unstaged_files = parse_diff(&repo, &unstaged_diff)?;

    // 3. Fetch staged changes (HEAD tree vs Index)
    let mut staged_files = Vec::new();
    if let Ok(head_ref) = repo.head() {
        if let Ok(head_tree) = head_ref.peel_to_tree() {
            let staged_diff = repo.diff_tree_to_index(Some(&head_tree), None, None)?;
            staged_files = parse_diff(&repo, &staged_diff)?;
        }
    }

    // 4. Calculate total summary counts
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    for f in &unstaged_files {
        total_insertions += f.insertions;
        total_deletions += f.deletions;
    }
    for f in &staged_files {
        total_insertions += f.insertions;
        total_deletions += f.deletions;
    }

    Ok(RepositoryDiff {
        has_repo: true,
        repo_name,
        total_insertions,
        total_deletions,
        unstaged_files,
        staged_files,
    })
}

/// Helper: Parse a `git2::Diff` structure into a vector of detailed `FileDiff` structures.
///
/// Hey buddy! libgit2 provides a Patch builder API which allows us to inspect
/// files, hunks, and lines easily. We iterate over all diff deltas to retrieve
/// patch data.
fn parse_diff(repo: &Repository, diff: &Diff) -> Result<Vec<FileDiff>, DiffError> {
    let mut file_diffs = Vec::new();
    let num_deltas = diff.deltas().len();

    for idx in 0..num_deltas {
        // Build a patch for each file change
        if let Ok(Some(patch)) = git2::Patch::from_diff(diff, idx) {
            let delta = patch.delta();
            
            // Extract the new path (or old path if it was deleted)
            let path = delta.new_file().path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();

            // Determine status label string
            let status = match delta.status() {
                git2::Delta::Added => "added",
                git2::Delta::Deleted => "deleted",
                git2::Delta::Modified => "modified",
                git2::Delta::Renamed => "renamed",
                git2::Delta::Typechange => "typechanged",
                git2::Delta::Untracked => "untracked",
                _ => "modified",
            }.to_string();

            // Fetch insertion and deletion line statistics for this patch
            let (insertions, deletions) = patch.line_stats()
                .map(|(ins, del, _)| (ins, del))
                .unwrap_or((0, 0));

            // Extract hunk modifications
            let mut hunks = Vec::new();
            let num_hunks = patch.num_hunks();
            
            for h_idx in 0..num_hunks {
                let (hunk, num_lines) = patch.hunk(h_idx)?;
                let header = String::from_utf8_lossy(hunk.header()).into_owned();

                let mut lines = Vec::new();
                for l_idx in 0..num_lines {
                    let line = patch.line_in_hunk(h_idx, l_idx)?;
                    let line_type = line.origin(); // Origin tells us if it's '+', '-', ' '
                    let content = String::from_utf8_lossy(line.content()).into_owned();

                    lines.push(DiffLine {
                        line_type,
                        content,
                        old_line_num: line.old_lineno(),
                        new_line_num: line.new_lineno(),
                    });
                }

                hunks.push(DiffHunk {
                    header,
                    lines,
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                });
            }

            file_diffs.push(FileDiff {
                path,
                status,
                insertions,
                deletions,
                hunks,
            });
        }
    }

    Ok(file_diffs)
}

/// Stages a modified or untracked file to the Git index.
pub fn stage_file<P: AsRef<Path>>(workspace_root: P, relative_path: &str) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    let mut index = repo.index()?;
    index.add_path(Path::new(relative_path))?;
    index.write()?;
    Ok(())
}

/// Unstages a file by resetting its index state back to HEAD.
pub fn unstage_file<P: AsRef<Path>>(workspace_root: P, relative_path: &str) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    
    // Check if HEAD commit exists (we can only reset if there's a head commit)
    match repo.head() {
        Ok(head_ref) => {
            let head_commit = head_ref.peel_to_commit()?;
            repo.reset_default(Some(head_commit.as_object()), &[Path::new(relative_path)])?;
        }
        Err(_) => {
            // Unborn branch / new repo - unstage by removing the file from the index
            let mut index = repo.index()?;
            let _ = index.remove_path(Path::new(relative_path));
            index.write()?;
        }
    }
    
    Ok(())
}

/// Reverts unstaged modifications to a file in the workdir by checking it out from the Index.
pub fn revert_file<P: AsRef<Path>>(workspace_root: P, relative_path: &str) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    
    // Check if the file is untracked. Reverting an untracked file means deleting it.
    let status = repo.status_file(Path::new(relative_path))?;
    if status.contains(git2::Status::WT_NEW) {
        let full_path = repo.workdir().unwrap_or_else(|| Path::new("")).join(relative_path);
        if full_path.exists() {
            std::fs::remove_file(full_path)?;
        }
        return Ok(());
    }

    // Otherwise, checkout the file from the index to overwrite changes
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force().path(Path::new(relative_path));
    repo.checkout_index(None, Some(&mut checkout_opts))?;
    
    Ok(())
}

/// Stages all modified, deleted, and untracked files in the repository.
pub fn stage_all_files<P: AsRef<Path>>(workspace_root: P) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    let mut index = repo.index()?;
    
    // Add all modifications and untracked files
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    
    Ok(())
}

/// Discards all unstaged changes in the workdir (except untracked files).
pub fn revert_all_files<P: AsRef<Path>>(workspace_root: P) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    
    // Force checkout from index to discard all unstaged edits to tracked files
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_index(None, Some(&mut checkout_opts))?;
    
    Ok(())
}
