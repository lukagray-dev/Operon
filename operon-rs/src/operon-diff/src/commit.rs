// commit.rs — Git Commit Creation Engine for operon-diff
//
// Hey friend! This module handles committing staged index changes to the repository history.
// It resolves user signatures, creates initial/unborn HEAD commits, handles commit amending,
// and maps signature errors to `DiffError::SignatureMissing` for clean frontend handling.

use crate::dto::CommitResult;
use crate::error::DiffError;
use crate::status::discover_repository;
use git2::Repository;
use std::path::Path;

/// Synchronously creates or amends a commit in the specified repository.
///
/// If no Git signature (`user.name` and `user.email`) is configured in the environment or Git config,
/// this function returns `DiffError::SignatureMissing` with a clear actionable error message.
pub fn commit(repo: &Repository, message: &str, amend: bool) -> Result<CommitResult, DiffError> {
    // 1. Resolve author & committer signature from Git config
    let sig = repo.signature().map_err(|e| {
        DiffError::SignatureMissing(format!(
            "Git signature missing (user.name or user.email not configured in git config): {e}"
        ))
    })?;

    // 2. Write current index state to a Git tree object
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // 3. Resolve parent commit(s) from HEAD if HEAD exists
    let head_ref = repo.head().ok();
    let parent_commit = head_ref.as_ref().and_then(|r| r.peel_to_commit().ok());

    // 4. Perform amend or fresh commit creation
    if amend {
        if let Some(parent) = parent_commit {
            let amended_oid = parent.amend(
                Some("HEAD"),
                Some(&sig),
                Some(&sig),
                None,
                Some(message),
                Some(&tree),
            )?;
            return Ok(CommitResult {
                oid: amended_oid.to_string(),
            });
        }
    }

    // Standard commit creation (initial unborn HEAD vs normal commit with parent)
    let oid = match parent_commit {
        Some(parent) => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])?,
        None => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[])?,
    };

    Ok(CommitResult {
        oid: oid.to_string(),
    })
}

/// Convenience overload accepting workspace directory path.
pub fn commit_workspace<P: AsRef<Path>>(
    workspace_root: P,
    message: &str,
    amend: bool,
) -> Result<CommitResult, DiffError> {
    let repo = discover_repository(workspace_root)?;
    commit(&repo, message, amend)
}
