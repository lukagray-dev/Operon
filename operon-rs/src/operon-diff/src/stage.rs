// stage.rs — Index & Staging Operations Engine for operon-diff
//
// Hey friend! This module provides functions to manipulate the Git Index (staging area)
// and working directory, enabling selective staging, unstaging, reverting, untracked file cleanup,
// and hunk-level patch application.

use std::path::Path;
use git2::{ApplyLocation, DiffOptions, StatusOptions};
use crate::diff::parse_diff;
use crate::error::DiffError;
use crate::status::discover_repository;

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
            repo.reset_default(Some(head_commit.as_object()), [Path::new(relative_path)])?;
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
            if full_path.is_dir() {
                std::fs::remove_dir_all(full_path)?;
            } else {
                std::fs::remove_file(full_path)?;
            }
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

/// Discards all unstaged changes in the workdir for tracked files.
pub fn revert_all_files<P: AsRef<Path>>(workspace_root: P) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    
    // Force checkout from index to discard all unstaged edits to tracked files
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_index(None, Some(&mut checkout_opts))?;
    
    Ok(())
}

/// Discards ALL unstaged modifications to tracked files AND permanently removes all untracked files/folders.
pub fn discard_all_including_untracked<P: AsRef<Path>>(workspace_root: P) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;

    // 1. Revert tracked changes from Index
    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force();
    repo.checkout_index(None, Some(&mut checkout_opts))?;

    // 2. Remove untracked files and directories
    let mut status_opts = StatusOptions::new();
    status_opts.include_untracked(true);
    status_opts.recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut status_opts))?;
    let workdir = repo.workdir().unwrap_or_else(|| Path::new("."));

    for entry in statuses.iter() {
        if entry.status().contains(git2::Status::WT_NEW) {
            if let Ok(path_str) = entry.path() {
                let full_path = workdir.join(path_str);
                if full_path.exists() {
                    if full_path.is_dir() {
                        let _ = std::fs::remove_dir_all(&full_path);
                    } else {
                        let _ = std::fs::remove_file(&full_path);
                        if let Some(parent) = full_path.parent() {
                            if parent != workdir && parent.starts_with(workdir) {
                                if let Ok(mut read_dir) = std::fs::read_dir(parent) {
                                    if read_dir.next().is_none() {
                                        let _ = std::fs::remove_dir(parent);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Stages a single hunk within a file by constructing a unified diff patch and applying it to the Git Index.
///
/// TODO(sub-hunk): Line-range selection within a hunk is reserved for a future enhancement pass.
pub fn stage_hunk<P: AsRef<Path>>(
    workspace_root: P,
    relative_path: &str,
    hunk_header: &str,
) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    
    // Fetch workdir diff for the target file
    let mut opts = DiffOptions::new();
    opts.pathspec(relative_path);
    let diff = repo.diff_index_to_workdir(None, Some(&mut opts))?;
    let file_diffs = parse_diff(&repo, &diff)?;

    let target_file = file_diffs.iter().find(|f| f.path == relative_path);
    if let Some(file_diff) = target_file {
        if let Some(hunk) = file_diff.hunks.iter().find(|h| h.header.trim() == hunk_header.trim() || h.header.contains(hunk_header)) {
            let patch_str = build_unified_patch(relative_path, hunk);
            let patch_diff = git2::Diff::from_buffer(patch_str.as_bytes())?;
            repo.apply(&patch_diff, ApplyLocation::Index, None)?;
            return Ok(());
        }
    }

    Err(DiffError::HeadResolution(format!(
        "Hunk matching '{hunk_header}' in file '{relative_path}' was not found"
    )))
}

/// Unstages a single hunk within a file by applying a reverse patch to the Git Index.
///
/// TODO(sub-hunk): Line-range selection within a hunk is reserved for a future enhancement pass.
pub fn unstage_hunk<P: AsRef<Path>>(
    workspace_root: P,
    relative_path: &str,
    hunk_header: &str,
) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;

    // Fetch staged diff (HEAD vs Index) for the target file
    let mut staged_files = Vec::new();
    if let Ok(head_ref) = repo.head() {
        if let Ok(head_tree) = head_ref.peel_to_tree() {
            let mut opts = DiffOptions::new();
            opts.pathspec(relative_path);
            let diff = repo.diff_tree_to_index(Some(&head_tree), None, Some(&mut opts))?;
            staged_files = parse_diff(&repo, &diff)?;
        }
    }

    let target_file = staged_files.iter().find(|f| f.path == relative_path);
    if let Some(file_diff) = target_file {
        if let Some(hunk) = file_diff.hunks.iter().find(|h| h.header.trim() == hunk_header.trim() || h.header.contains(hunk_header)) {
            let reverse_patch_str = build_reverse_unified_patch(relative_path, hunk);
            let patch_diff = git2::Diff::from_buffer(reverse_patch_str.as_bytes())?;
            repo.apply(&patch_diff, ApplyLocation::Index, None)?;
            return Ok(());
        }
    }

    Err(DiffError::HeadResolution(format!(
        "Staged hunk matching '{hunk_header}' in file '{relative_path}' was not found"
    )))
}

/// Helper function to build a unified patch string for a single hunk.
fn build_unified_patch(relative_path: &str, hunk: &crate::dto::DiffHunk) -> String {
    let mut patch = String::new();
    patch.push_str(&format!("diff --git a/{relative_path} b/{relative_path}\n"));
    patch.push_str(&format!("--- a/{relative_path}\n"));
    patch.push_str(&format!("+++ b/{relative_path}\n"));
    patch.push_str(&format!("{}\n", hunk.header.trim_end()));
    for line in &hunk.lines {
        patch.push(line.line_type);
        patch.push_str(&line.content);
        if !line.content.ends_with('\n') {
            patch.push('\n');
        }
    }
    patch
}

/// Helper function to build a reverse unified patch string for unstaging a single hunk.
fn build_reverse_unified_patch(relative_path: &str, hunk: &crate::dto::DiffHunk) -> String {
    let mut patch = String::new();
    patch.push_str(&format!("diff --git a/{relative_path} b/{relative_path}\n"));
    patch.push_str(&format!("--- a/{relative_path}\n"));
    patch.push_str(&format!("+++ b/{relative_path}\n"));
    
    // Invert header ranges for reverse patch
    patch.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        hunk.new_start, hunk.new_lines, hunk.old_start, hunk.old_lines
    ));

    for line in &hunk.lines {
        let rev_type = match line.line_type {
            '+' => '-',
            '-' => '+',
            other => other,
        };
        patch.push(rev_type);
        patch.push_str(&line.content);
        if !line.content.ends_with('\n') {
            patch.push('\n');
        }
    }
    patch
}
