// repo_manager.rs — Multi-Repository Workspace Registry for operon-diff
//
// Hey friend! This module provides `RepoRegistry`, an in-memory manager that discovers,
// tracks, and switches between multiple Git repositories located within a workspace folder.

use crate::dto::RepoEntry;
use crate::error::DiffError;
use crate::status::{discover_repository, get_diff_stats};
use std::path::{Path, PathBuf};

/// In-memory manager tracking all discovered repositories in a multi-repository workspace.
#[derive(Debug, Clone, Default)]
pub struct RepoRegistry {
    repos: Vec<RepoEntry>,
    active_root: Option<PathBuf>,
}

impl RepoRegistry {
    /// Creates a new empty `RepoRegistry`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans `workspace_root` itself and its immediate subdirectories (non-recursive) for `.git` folders/files.
    ///
    /// Populates and returns the list of discovered repositories.
    pub fn discover_workspace_repos<P: AsRef<Path>>(
        &mut self,
        workspace_root: P,
    ) -> Vec<RepoEntry> {
        let root = workspace_root.as_ref();
        let mut candidates = Vec::new();

        // 1. Check workspace_root itself
        if root.exists() && root.is_dir() {
            candidates.push(root.to_path_buf());
        }

        // 2. Scan immediate subdirectories
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    candidates.push(path);
                }
            }
        }

        // 3. Filter valid git repositories and build RepoEntry list
        let mut discovered = Vec::new();

        for path in candidates {
            let git_entry = path.join(".git");
            if git_entry.exists() {
                if let Ok(repo) = discover_repository(&path) {
                    let name = repo
                        .workdir()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("repository")
                        .to_string();

                    let has_changes = get_diff_stats(&path)
                        .map(|s| s.insertions > 0 || s.deletions > 0)
                        .unwrap_or(false);

                    let canonical_path = path.canonicalize().unwrap_or(path.clone());

                    let is_active = match &self.active_root {
                        Some(act) => act == &canonical_path || act == &path,
                        None => discovered.is_empty(), // default first discovered to active if none set
                    };

                    let entry = RepoEntry {
                        root: path.clone(),
                        name,
                        is_active,
                        has_changes,
                    };

                    if is_active && self.active_root.is_none() {
                        self.active_root = Some(path.clone());
                    }

                    discovered.push(entry);
                }
            }
        }

        self.repos = discovered.clone();
        discovered
    }

    /// Adds a single repository manually by its root path.
    pub fn add_repo<P: AsRef<Path>>(&mut self, root: P) -> Result<RepoEntry, DiffError> {
        let path = root.as_ref();
        let repo = discover_repository(path)?;

        let name = repo
            .workdir()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("repository")
            .to_string();

        let has_changes = get_diff_stats(path)
            .map(|s| s.insertions > 0 || s.deletions > 0)
            .unwrap_or(false);

        let is_active = self.repos.is_empty();
        let entry = RepoEntry {
            root: path.to_path_buf(),
            name,
            is_active,
            has_changes,
        };

        if is_active {
            self.active_root = Some(path.to_path_buf());
        }

        // Avoid duplicate entries
        if !self.repos.iter().any(|r| r.root == path) {
            self.repos.push(entry.clone());
        }

        Ok(entry)
    }

    /// Removes a repository entry from the registry.
    pub fn remove_repo<P: AsRef<Path>>(&mut self, root: P) -> bool {
        let path = root.as_ref();
        let initial_len = self.repos.len();
        self.repos.retain(|r| r.root != path);

        if self.active_root.as_deref() == Some(path) {
            self.active_root = self.repos.first().map(|r| r.root.clone());
            if let Some(first) = self.repos.first_mut() {
                first.is_active = true;
            }
        }

        self.repos.len() < initial_len
    }

    /// Sets the active repository root by matching path.
    pub fn set_active<P: AsRef<Path>>(&mut self, root: P) -> Result<(), DiffError> {
        let path = root.as_ref();
        let found = self.repos.iter().any(|r| r.root == path);
        if !found {
            return Err(DiffError::RepoNotFound(format!(
                "Repository at '{path:?}' is not tracked in registry"
            )));
        }

        self.active_root = Some(path.to_path_buf());
        for repo in &mut self.repos {
            repo.is_active = repo.root == path;
        }

        Ok(())
    }

    /// Returns the currently active repository entry, if any.
    pub fn active_repo(&self) -> Option<&RepoEntry> {
        self.repos.iter().find(|r| r.is_active)
    }

    /// Returns all registered repository entries.
    pub fn list_repos(&self) -> Vec<RepoEntry> {
        self.repos.clone()
    }
}
