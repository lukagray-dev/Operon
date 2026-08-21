// status.rs — Repository Discovery & Diff Statistics Engine for operon-diff
//
// Hey friend! This module handles discovery of Git repository roots and calculates workspace-wide
// diff statistics (insertion/deletion counts) as well as full staged and unstaged file trees.

use crate::diff::parse_diff;
use crate::dto::{GitDiffStats, RepositoryDiff};
use crate::error::DiffError;
use git2::{DiffOptions, Repository};
use std::path::Path;

/// Helper: Discovers the repository root from the given workspace folder path.
///
/// Hey buddy! If the folder passed doesn't have a `.git` folder directly inside it,
/// libgit2 will search parent directories until it finds the repository root.
pub fn discover_repository<P: AsRef<Path>>(workspace_root: P) -> Result<Repository, DiffError> {
    let repo =
        Repository::discover(workspace_root).map_err(|e| DiffError::NoRepository(e.to_string()))?;
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

    // 2. Fetch staged stats (Index vs HEAD tree, handling unborn HEAD)
    let (staged_ins, staged_del) = {
        let head_tree = repo.head().ok().and_then(|r| r.peel_to_tree().ok());
        if let Ok(diff) = repo.diff_tree_to_index(head_tree.as_ref(), None, None) {
            if let Ok(stats) = diff.stats() {
                (stats.insertions(), stats.deletions())
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        }
    };

    // 3. Fetch unstaged stats (Workdir vs Index). We include untracked files!
    let mut opts = DiffOptions::new();
    opts.include_untracked(true);
    opts.recurse_untracked_dirs(true);
    opts.show_untracked_content(true);

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
    let repo_name = repo
        .workdir()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_string();

    // 2. Fetch unstaged changes (Index vs Workdir)
    let mut unstaged_opts = DiffOptions::new();
    unstaged_opts.include_untracked(true);
    unstaged_opts.recurse_untracked_dirs(true);
    unstaged_opts.show_untracked_content(true);
    let unstaged_diff = repo.diff_index_to_workdir(None, Some(&mut unstaged_opts))?;
    let unstaged_files = parse_diff(&repo, &unstaged_diff)?;

    // 3. Fetch staged changes (HEAD tree vs Index, handling unborn HEAD)
    let head_tree = repo.head().ok().and_then(|r| r.peel_to_tree().ok());
    let staged_diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
    let staged_files = parse_diff(&repo, &staged_diff)?;

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
