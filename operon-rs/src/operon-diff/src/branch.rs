// branch.rs — Git Branch Management Engine for operon-diff
//
// Hey friend! This module implements all local branch operations including retrieving the current active branch,
// listing local branches with upstream ahead/behind tracking metrics, creating new branches, switching (checkout),
// renaming, and deleting branches.

use std::path::Path;
use git2::{BranchType, Repository};
use crate::dto::BranchInfo;
use crate::error::DiffError;
use crate::status::discover_repository;

/// Retrieves detailed information for the currently checked out branch (HEAD).
pub fn current_branch(repo: &Repository) -> Result<BranchInfo, DiffError> {
    let head_ref = match repo.head() {
        Ok(r) => r,
        Err(_) => {
            // Unborn branch scenario (e.g. fresh repository with no commits yet)
            let branch_name = repo
                .find_reference("HEAD")
                .ok()
                .and_then(|r| r.symbolic_target().ok().flatten().map(|s| s.to_string()))
                .and_then(|target| target.strip_prefix("refs/heads/").map(|s| s.to_string()))
                .unwrap_or_else(|| "main".to_string());

            return Ok(BranchInfo {
                name: branch_name,
                is_head: true,
                upstream: None,
                ahead: 0,
                behind: 0,
            });
        }
    };

    let name = head_ref
        .shorthand()
        .unwrap_or("HEAD")
        .to_string();

    let local_oid = head_ref.target();

    let mut upstream_name = None;
    let mut ahead = 0;
    let mut behind = 0;

    if head_ref.is_branch() {
        if let Ok(branch) = repo.find_branch(&name, BranchType::Local) {
            if let Ok(upstream) = branch.upstream() {
                if let Ok(up_name) = upstream.name() {
                    upstream_name = up_name.map(|s| s.to_string());
                }

                let upstream_oid = upstream.get().target();
                if let (Some(l_oid), Some(u_oid)) = (local_oid, upstream_oid) {
                    if let Ok((ah, bh)) = repo.graph_ahead_behind(l_oid, u_oid) {
                        ahead = ah;
                        behind = bh;
                    }
                }
            }
        }
    }

    Ok(BranchInfo {
        name,
        is_head: true,
        upstream: upstream_name,
        ahead,
        behind,
    })
}

/// Lists all local branches in the repository with ahead/behind metrics relative to their configured upstreams.
pub fn list_branches(repo: &Repository) -> Result<Vec<BranchInfo>, DiffError> {
    let current = current_branch(repo).ok();
    let current_name = current.as_ref().map(|c| c.name.as_str());

    let mut branches_list = Vec::new();
    let branches = repo.branches(Some(BranchType::Local))?;

    for entry in branches {
        let (branch, _) = entry?;
        let name = match branch.name()? {
            Some(n) => n.to_string(),
            None => continue,
        };

        let is_head = current_name == Some(&name);

        let local_oid = branch.get().target();
        let mut upstream_name = None;
        let mut ahead = 0;
        let mut behind = 0;

        if let Ok(upstream) = branch.upstream() {
            if let Ok(up_name) = upstream.name() {
                upstream_name = up_name.map(|s| s.to_string());
            }

            let upstream_oid = upstream.get().target();
            if let (Some(l_oid), Some(u_oid)) = (local_oid, upstream_oid) {
                if let Ok((ah, bh)) = repo.graph_ahead_behind(l_oid, u_oid) {
                    ahead = ah;
                    behind = bh;
                }
            }
        }

        branches_list.push(BranchInfo {
            name,
            is_head,
            upstream: upstream_name,
            ahead,
            behind,
        });
    }

    Ok(branches_list)
}

/// Creates a new local branch pointing at `target_commit_sha` (or HEAD if None).
pub fn create_branch(
    repo: &Repository,
    name: &str,
    target_commit_sha: Option<&str>,
) -> Result<BranchInfo, DiffError> {
    let commit = match target_commit_sha {
        Some(sha) => {
            let oid = git2::Oid::from_str(sha)?;
            repo.find_commit(oid)?
        }
        None => {
            let head_ref = repo.head()?;
            head_ref.peel_to_commit()?
        }
    };

    let branch = repo.branch(name, &commit, false)?;
    let local_oid = branch.get().target();

    let mut upstream_name = None;
    let mut ahead = 0;
    let mut behind = 0;

    if let Ok(upstream) = branch.upstream() {
        if let Ok(up_name) = upstream.name() {
            upstream_name = up_name.map(|s| s.to_string());
        }
        let upstream_oid = upstream.get().target();
        if let (Some(l_oid), Some(u_oid)) = (local_oid, upstream_oid) {
            if let Ok((ah, bh)) = repo.graph_ahead_behind(l_oid, u_oid) {
                ahead = ah;
                behind = bh;
            }
        }
    }

    Ok(BranchInfo {
        name: name.to_string(),
        is_head: false,
        upstream: upstream_name,
        ahead,
        behind,
    })
}

/// Switches HEAD to the specified branch name and updates the working directory.
pub fn switch_branch(repo: &Repository, name: &str) -> Result<(), DiffError> {
    let ref_name = format!("refs/heads/{name}");
    repo.find_reference(&ref_name)
        .map_err(|_| DiffError::BranchNotFound(format!("Branch '{name}' does not exist")))?;

    repo.set_head(&ref_name)?;

    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.safe();
    repo.checkout_head(Some(&mut checkout_opts))?;

    Ok(())
}

/// Deletes the specified local branch from the repository.
pub fn delete_branch(repo: &Repository, name: &str) -> Result<(), DiffError> {
    let mut branch = repo.find_branch(name, BranchType::Local)
        .map_err(|_| DiffError::BranchNotFound(format!("Branch '{name}' does not exist")))?;
    
    branch.delete()?;
    Ok(())
}

/// Renames an existing local branch.
pub fn rename_branch(repo: &Repository, old_name: &str, new_name: &str) -> Result<(), DiffError> {
    let mut branch = repo.find_branch(old_name, BranchType::Local)
        .map_err(|_| DiffError::BranchNotFound(format!("Branch '{old_name}' does not exist")))?;

    branch.rename(new_name, false)?;
    Ok(())
}

/// Convenience workspace path overloads
pub fn current_branch_workspace<P: AsRef<Path>>(workspace_root: P) -> Result<BranchInfo, DiffError> {
    let repo = discover_repository(workspace_root)?;
    current_branch(&repo)
}

pub fn list_branches_workspace<P: AsRef<Path>>(workspace_root: P) -> Result<Vec<BranchInfo>, DiffError> {
    let repo = discover_repository(workspace_root)?;
    list_branches(&repo)
}

pub fn create_branch_workspace<P: AsRef<Path>>(
    workspace_root: P,
    name: &str,
    target_commit_sha: Option<&str>,
) -> Result<BranchInfo, DiffError> {
    let repo = discover_repository(workspace_root)?;
    create_branch(&repo, name, target_commit_sha)
}

pub fn switch_branch_workspace<P: AsRef<Path>>(workspace_root: P, name: &str) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    switch_branch(&repo, name)
}

pub fn delete_branch_workspace<P: AsRef<Path>>(workspace_root: P, name: &str) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    delete_branch(&repo, name)
}

pub fn rename_branch_workspace<P: AsRef<Path>>(
    workspace_root: P,
    old_name: &str,
    new_name: &str,
) -> Result<(), DiffError> {
    let repo = discover_repository(workspace_root)?;
    rename_branch(&repo, old_name, new_name)
}
