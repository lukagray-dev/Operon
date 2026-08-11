// remote.rs — Git Remote Operations Engine (Push/Fetch/Pull) for operon-diff
//
// Hey friend! This module handles network operations with remote repositories (push, fetch, pull).
// Credentials (SSH agent, SSH keys, HTTPS tokens) are automatically managed using `auth-git2`.
// Pull operations enforce fast-forward-only updates and return `DiffError::MergeConflict` if non-fast-forwardable.

use std::path::Path;
use git2::Repository;
use crate::error::DiffError;
use crate::status::discover_repository;

/// Pushes local branch commits to the specified remote repository.
pub fn push(repo: &Repository, remote_name: &str, branch: &str) -> Result<(), DiffError> {
    let mut remote = repo.find_remote(remote_name)
        .map_err(|e| DiffError::RemoteAuth(format!("Remote '{remote_name}' not found: {e}")))?;

    let auth = auth_git2::GitAuthenticator::default();
    let config = repo.config()?;
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(auth.credentials(&config));

    let mut push_opts = git2::PushOptions::new();
    push_opts.remote_callbacks(callbacks);

    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[&refspec], Some(&mut push_opts))
        .map_err(|e| DiffError::RemoteAuth(format!("Push to '{remote_name}/{branch}' failed: {e}")))?;

    Ok(())
}

/// Fetches branch refs and objects from the specified remote repository.
pub fn fetch(repo: &Repository, remote_name: &str) -> Result<(), DiffError> {
    let mut remote = repo.find_remote(remote_name)
        .map_err(|e| DiffError::RemoteAuth(format!("Remote '{remote_name}' not found: {e}")))?;

    let auth = auth_git2::GitAuthenticator::default();
    let config = repo.config()?;
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(auth.credentials(&config));

    let mut fetch_opts = git2::FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);

    remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)
        .map_err(|e| DiffError::RemoteAuth(format!("Fetch from '{remote_name}' failed: {e}")))?;

    Ok(())
}

/// Pulls changes from remote by performing a fetch followed by a fast-forward-only merge.
///
/// If the merge is non-fast-forwardable, returns `DiffError::MergeConflict` without modifying state.
pub fn pull(repo: &Repository, remote_name: &str, branch: &str) -> Result<(), DiffError> {
    // 1. Fetch remote changes first
    fetch(repo, remote_name)?;

    // 2. Resolve target remote branch reference
    let remote_ref_name = format!("refs/remotes/{remote_name}/{branch}");
    let fetch_head = repo.find_reference(&remote_ref_name)
        .map_err(|_| DiffError::BranchNotFound(format!("Remote tracking branch '{remote_ref_name}' not found")))?;

    let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

    // 3. Perform merge analysis
    let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

    if analysis.is_up_to_date() {
        return Ok(());
    }

    if analysis.is_fast_forward() {
        let target_oid = fetch_commit.id();
        let target_commit = repo.find_commit(target_oid)?;

        // Checkout target commit tree into working directory
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();
        repo.checkout_tree(target_commit.as_object(), Some(&mut checkout_opts))?;

        // Fast-forward local HEAD reference
        let mut head_ref = repo.head()?;
        head_ref.set_target(target_oid, &format!("Fast-forward pull from {remote_name}/{branch}"))?;

        Ok(())
    } else {
        Err(DiffError::MergeConflict(format!(
            "Cannot fast-forward branch '{branch}' from '{remote_name}'. Non-fast-forward merge required."
        )))
    }
}

/// Convenience workspace path overloads.
pub fn push_workspace<P: AsRef<Path>>(
    workspace_root: P,
    remote_name: &str,
    branch: &str,
) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    push(&repo, remote_name, branch)
}

pub fn fetch_workspace<P: AsRef<Path>>(workspace_root: P, remote_name: &str) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    fetch(&repo, remote_name)
}

pub fn pull_workspace<P: AsRef<Path>>(
    workspace_root: P,
    remote_name: &str,
    branch: &str,
) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    pull(&repo, remote_name, branch)
}
