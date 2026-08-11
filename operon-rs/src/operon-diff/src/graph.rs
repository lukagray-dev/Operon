// graph.rs — Visual Commit Graph & Unpushed Commit Detector for operon-diff
//
// Hey friend! This module provides topological and chronological commit graph traversal via `git2::Revwalk`,
// resolves branch tag badges for commit nodes, and computes unpushed local commit sets.

use std::collections::{HashSet, HashMap};
use std::path::Path;
use git2::{BranchType, Oid, Repository, Sort};
use crate::dto::GitGraphCommit;
use crate::error::DiffError;
use crate::status::discover_repository;

/// Computes paginated commit graph history for visual rendering in Slint UI.
pub fn get_commit_graph(
    repo: &Repository,
    limit: usize,
    skip: usize,
) -> Result<Vec<GitGraphCommit>, DiffError> {
    // 1. Resolve HEAD commit OID if present
    let head_oid = repo.head().ok().and_then(|r| r.target());

    // 2. Pre-build branch tag cache: map Oid -> comma-separated branch names
    let branch_map = build_branch_tag_map(repo);

    // 3. Compute unpushed (local-only) OID set
    let unpushed_set = compute_unpushed_set(repo);

    // 4. Initialize Revwalk with topological and time sorting
    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(_) => return Ok(Vec::new()),
    };

    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

    // Push local branch references or HEAD
    if revwalk.push_glob("refs/heads/*").is_err() {
        if let Some(oid) = head_oid {
            let _ = revwalk.push(oid);
        }
    }

    // 5. Collect paginated commit nodes
    let mut commits = Vec::new();
    let mut current_idx = 0;

    for oid_res in revwalk {
        let oid = match oid_res {
            Ok(o) => o,
            Err(_) => continue,
        };

        if current_idx < skip {
            current_idx += 1;
            continue;
        }

        if limit > 0 && commits.len() >= limit {
            break;
        }

        if let Ok(commit) = repo.find_commit(oid) {
            let hash = oid.to_string();
            let short_hash = if hash.len() >= 7 { hash[..7].to_string() } else { hash.clone() };

            let message = match commit.summary() {
                Ok(Some(s)) => s.to_string(),
                _ => match commit.message() {
                    Ok(m) => m.to_string(),
                    Err(_) => String::new(),
                },
            };
            let author = commit.author().name().unwrap_or(commit.author().email().unwrap_or("Unknown")).to_string();
            let branch_tag = branch_map.get(&oid).cloned().unwrap_or_default();
            let is_head = head_oid == Some(oid);
            let is_local = unpushed_set.contains(&oid);

            commits.push(GitGraphCommit {
                hash,
                short_hash,
                message,
                author,
                branch_tag,
                is_head,
                is_local,
            });
        }

        current_idx += 1;
    }

    Ok(commits)
}

/// Resolves which local branch tip(s) point at the given commit OID.
pub fn branch_tag_for(repo: &Repository, oid: Oid) -> String {
    let branch_map = build_branch_tag_map(repo);
    branch_map.get(&oid).cloned().unwrap_or_default()
}

/// Helper: Builds a map of Commit OID -> comma-separated branch names pointing to that commit.
fn build_branch_tag_map(repo: &Repository) -> HashMap<Oid, String> {
    let mut map: HashMap<Oid, Vec<String>> = HashMap::new();

    if let Ok(branches) = repo.branches(Some(BranchType::Local)) {
        for entry in branches.flatten() {
            let (branch, _) = entry;
            if let (Ok(Some(name)), Some(target)) = (branch.name(), branch.get().target()) {
                map.entry(target).or_default().push(name.to_string());
            }
        }
    }

    map.into_iter()
        .map(|(oid, names)| (oid, names.join(", ")))
        .collect()
}

/// Helper: Computes the set of commit OIDs that are local-only (unpushed to remote).
///
/// For the current branch, computes the OID set in `local..upstream` range via revwalk with `.hide(upstream_oid)`.
fn compute_unpushed_set(repo: &Repository) -> HashSet<Oid> {
    let mut unpushed = HashSet::new();

    let head_ref = match repo.head() {
        Ok(r) => r,
        Err(_) => return unpushed,
    };

    let local_oid = match head_ref.target() {
        Some(o) => o,
        None => return unpushed,
    };

    if !head_ref.is_branch() {
        return unpushed;
    }

    let branch_name = match head_ref.shorthand() {
        Ok(n) => n,
        Err(_) => return unpushed,
    };

    let branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return unpushed,
    };

    if let Ok(upstream) = branch.upstream() {
        if let Some(upstream_oid) = upstream.get().target() {
            if let Ok(mut walk) = repo.revwalk() {
                let _ = walk.push(local_oid);
                let _ = walk.hide(upstream_oid);
                for oid in walk.flatten() {
                    unpushed.insert(oid);
                }
            }
        }
    } else {
        // No upstream configured for this branch: all commits reachable from HEAD that are not on remote branches are local
        if let Ok(mut walk) = repo.revwalk() {
            let _ = walk.push(local_oid);
            // Hide all remote tracking branch tips
            if let Ok(remote_branches) = repo.branches(Some(BranchType::Remote)) {
                for entry in remote_branches.flatten() {
                    let (r_branch, _) = entry;
                    if let Some(r_oid) = r_branch.get().target() {
                        let _ = walk.hide(r_oid);
                    }
                }
            }
            for oid in walk.flatten() {
                unpushed.insert(oid);
            }
        }
    }

    unpushed
}

/// Convenience workspace path overload.
pub fn get_commit_graph_workspace<P: AsRef<Path>>(
    workspace_root: P,
    limit: usize,
    skip: usize,
) -> Result<Vec<GitGraphCommit>, DiffError> {
    let repo = discover_repository(workspace_root)?;
    get_commit_graph(&repo, limit, skip)
}
