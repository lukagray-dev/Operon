use std::path::Path;

use git2::{ErrorCode, Repository, Status, StatusOptions};

use crate::error::SnapshotError;
use crate::types::GitStatus;

/// Builds git status data if `root` lives inside a git repository.
pub(crate) fn read_git_status(root: &Path) -> Result<Option<GitStatus>, SnapshotError> {
    let repo = match Repository::discover(root) {
        Ok(repo) => repo,
        Err(err)
            if matches!(
                err.code(),
                ErrorCode::NotFound | ErrorCode::Owner | ErrorCode::Invalid
            ) =>
        {
            return Ok(None);
        }
        Err(err) => return Err(err.into()),
    };

    let branch = current_branch_name(&repo);

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo.statuses(Some(&mut options))?;

    let mut staged = 0_usize;
    let mut unstaged = 0_usize;
    let mut untracked = 0_usize;

    for entry in statuses.iter() {
        let status = entry.status();

        if status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED,
        ) {
            staged = staged.saturating_add(1);
        }

        if status.intersects(Status::WT_MODIFIED | Status::WT_DELETED) {
            unstaged = unstaged.saturating_add(1);
        }

        if status.contains(Status::WT_NEW) {
            untracked = untracked.saturating_add(1);
        }
    }

    let (insertions, deletions) = modified_line_count(&repo)?;

    Ok(Some(GitStatus {
        branch,
        staged,
        unstaged,
        untracked,
        insertions,
        deletions,
    }))
}

fn current_branch_name(repo: &Repository) -> String {
    match repo.head() {
        Ok(head) => head
            .shorthand()
            .map(|name| name.to_string())
            .unwrap_or_else(|_| "DETACHED".to_string()),
        Err(_) => "HEAD".to_string(),
    }
}

fn modified_line_count(repo: &Repository) -> Result<(u64, u64), SnapshotError> {
    // Staged changes: index vs HEAD tree.
    // On a brand-new repo with no commits, head() fails â€” treat as zero.
    let (staged_insertions, staged_deletions) = match repo.head() {
        Ok(head_ref) => {
            let head_tree = head_ref.peel_to_tree()?;
            let diff = repo.diff_tree_to_index(Some(&head_tree), None, None)?;
            let stats = diff.stats()?;
            (
                usize_to_u64(stats.insertions()),
                usize_to_u64(stats.deletions()),
            )
        }
        Err(_) => (0, 0),
    };

    // Unstaged changes: workdir vs index.
    let workdir_diff = repo.diff_index_to_workdir(None, None)?;
    let workdir_stats = workdir_diff.stats()?;
    let workdir_insertions = usize_to_u64(workdir_stats.insertions());
    let workdir_deletions = usize_to_u64(workdir_stats.deletions());

    Ok((
        staged_insertions.saturating_add(workdir_insertions),
        staged_deletions.saturating_add(workdir_deletions),
    ))
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
