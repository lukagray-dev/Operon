// workspace.rs — Non-Blocking Async Wrappers for GUI Interop
//
// Hey friend! Because libgit2 operations are synchronous and blocking, running them directly
// on the Slint UI thread would cause frame drops or UI freezes.
//
// This module provides async `tokio::task::spawn_blocking` wrappers for every single public operation
// in the crate (suffixed with `_async`). The Slint desktop UI layer MUST call these functions!

use std::path::PathBuf;
use tokio::task;

use crate::dto::{BranchInfo, CommitResult, GitDiffStats, GitGraphCommit, RepoEntry, RepositoryDiff};
use crate::error::DiffError;
use crate::{branch, commit, graph, remote, repo_manager, stage, status};

/// Async non-blocking wrapper for `status::get_diff_stats`.
pub async fn get_diff_stats_async(workspace_root: PathBuf) -> Result<GitDiffStats, DiffError> {
    task::spawn_blocking(move || status::get_diff_stats(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `status::get_diff_details`.
pub async fn get_diff_details_async(workspace_root: PathBuf) -> Result<RepositoryDiff, DiffError> {
    task::spawn_blocking(move || status::get_diff_details(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `stage::stage_file`.
pub async fn stage_file_async(workspace_root: PathBuf, relative_path: String) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::stage_file(workspace_root, &relative_path))
        .await?
}

/// Async non-blocking wrapper for `stage::unstage_file`.
pub async fn unstage_file_async(workspace_root: PathBuf, relative_path: String) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::unstage_file(workspace_root, &relative_path))
        .await?
}

/// Async non-blocking wrapper for `stage::revert_file`.
pub async fn revert_file_async(workspace_root: PathBuf, relative_path: String) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::revert_file(workspace_root, &relative_path))
        .await?
}

/// Async non-blocking wrapper for `stage::stage_all_files`.
pub async fn stage_all_files_async(workspace_root: PathBuf) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::stage_all_files(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `stage::revert_all_files`.
pub async fn revert_all_files_async(workspace_root: PathBuf) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::revert_all_files(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `stage::discard_all_including_untracked`.
pub async fn discard_all_including_untracked_async(workspace_root: PathBuf) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::discard_all_including_untracked(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `stage::stage_hunk`.
pub async fn stage_hunk_async(
    workspace_root: PathBuf,
    relative_path: String,
    hunk_header: String,
) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::stage_hunk(workspace_root, &relative_path, &hunk_header))
        .await?
}

/// Async non-blocking wrapper for `stage::unstage_hunk`.
pub async fn unstage_hunk_async(
    workspace_root: PathBuf,
    relative_path: String,
    hunk_header: String,
) -> Result<(), DiffError> {
    task::spawn_blocking(move || stage::unstage_hunk(workspace_root, &relative_path, &hunk_header))
        .await?
}

/// Async non-blocking wrapper for `commit::commit`.
pub async fn commit_async(
    workspace_root: PathBuf,
    message: String,
    amend: bool,
) -> Result<CommitResult, DiffError> {
    task::spawn_blocking(move || commit::commit_workspace(workspace_root, &message, amend))
        .await?
}

/// Async non-blocking wrapper for `branch::current_branch`.
pub async fn current_branch_async(workspace_root: PathBuf) -> Result<BranchInfo, DiffError> {
    task::spawn_blocking(move || branch::current_branch_workspace(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `branch::list_branches`.
pub async fn list_branches_async(workspace_root: PathBuf) -> Result<Vec<BranchInfo>, DiffError> {
    task::spawn_blocking(move || branch::list_branches_workspace(workspace_root))
        .await?
}

/// Async non-blocking wrapper for `branch::create_branch`.
pub async fn create_branch_async(
    workspace_root: PathBuf,
    name: String,
    target_commit_sha: Option<String>,
) -> Result<BranchInfo, DiffError> {
    task::spawn_blocking(move || {
        branch::create_branch_workspace(workspace_root, &name, target_commit_sha.as_deref())
    })
    .await?
}

/// Async non-blocking wrapper for `branch::switch_branch`.
pub async fn switch_branch_async(workspace_root: PathBuf, name: String) -> Result<(), DiffError> {
    task::spawn_blocking(move || branch::switch_branch_workspace(workspace_root, &name))
        .await?
}

/// Async non-blocking wrapper for `branch::delete_branch`.
pub async fn delete_branch_async(workspace_root: PathBuf, name: String) -> Result<(), DiffError> {
    task::spawn_blocking(move || branch::delete_branch_workspace(workspace_root, &name))
        .await?
}

/// Async non-blocking wrapper for `branch::rename_branch`.
pub async fn rename_branch_async(
    workspace_root: PathBuf,
    old_name: String,
    new_name: String,
) -> Result<(), DiffError> {
    task::spawn_blocking(move || branch::rename_branch_workspace(workspace_root, &old_name, &new_name))
        .await?
}

/// Async non-blocking wrapper for `graph::get_commit_graph`.
pub async fn get_commit_graph_async(
    workspace_root: PathBuf,
    limit: usize,
    skip: usize,
) -> Result<Vec<GitGraphCommit>, DiffError> {
    task::spawn_blocking(move || graph::get_commit_graph_workspace(workspace_root, limit, skip))
        .await?
}

/// Async non-blocking wrapper for `remote::push`.
pub async fn push_async(
    workspace_root: PathBuf,
    remote_name: String,
    branch: String,
) -> Result<(), DiffError> {
    task::spawn_blocking(move || remote::push_workspace(workspace_root, &remote_name, &branch))
        .await?
}

/// Async non-blocking wrapper for `remote::fetch`.
pub async fn fetch_async(workspace_root: PathBuf, remote_name: String) -> Result<(), DiffError> {
    task::spawn_blocking(move || remote::fetch_workspace(workspace_root, &remote_name))
        .await?
}

/// Async non-blocking wrapper for `remote::pull`.
pub async fn pull_async(
    workspace_root: PathBuf,
    remote_name: String,
    branch: String,
) -> Result<(), DiffError> {
    task::spawn_blocking(move || remote::pull_workspace(workspace_root, &remote_name, &branch))
        .await?
}

/// Async non-blocking wrapper for `repo_manager::discover_workspace_repos`.
pub async fn discover_workspace_repos_async(workspace_root: PathBuf) -> Result<Vec<RepoEntry>, DiffError> {
    task::spawn_blocking(move || {
        let mut registry = repo_manager::RepoRegistry::new();
        Ok(registry.discover_workspace_repos(workspace_root))
    })
    .await?
}
