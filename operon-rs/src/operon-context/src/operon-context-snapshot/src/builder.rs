use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::blocks;
use crate::error::SnapshotError;
use crate::types::{DirectoryTree, Role, SessionSnapshot};

/// Runtime configuration for snapshot generation.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    pub root: PathBuf,
    pub role: Role,
    pub session_id: String,
    pub tree_depth: usize,
    /// Names of available built-in tool groups, in display order.
    /// Populated by whoever constructs the builder (the session runner).
    /// Example: vec!["fs", "shell", "web", "todo", "memory", "media"]
    /// If empty, the 5th block is omitted from the snapshot entirely.
    pub tool_groups: Vec<String>,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            root,
            role: Role::External,
            session_id: generate_session_id(),
            tree_depth: 1,
            tool_groups: Vec::new(),
        }
    }
}

/// Main entry point for per-turn snapshot construction.
///
/// This builder owns two memoized blocks (`AGENTS.md`, tree) and a live
/// filesystem watcher that flips dirty flags when relevant files change.
///
/// # Thread safety
///
/// `SnapshotBuilder` is `Send` but not `Sync`. Each session owns exactly one
/// builder. In async or multi-threaded contexts, wrap in `Mutex<SnapshotBuilder>`
/// before sharing across tasks.
pub struct SnapshotBuilder {
    config: SnapshotConfig,
    cached_tree: Option<DirectoryTree>,
    cached_agents_md: Option<Option<String>>,
    tree_dirty: Arc<AtomicBool>,
    agents_md_dirty: Arc<AtomicBool>,
    _watcher: RecommendedWatcher,
}

impl SnapshotBuilder {
    /// Constructs a new builder and starts a watcher for cache invalidation.
    pub fn new(mut config: SnapshotConfig) -> Result<Self, SnapshotError> {
        if !config.root.exists() {
            return Err(SnapshotError::InvalidRoot(config.root));
        }

        // Canonicalizing once avoids path-identity drift between calls.
        let mut root = config.root.canonicalize()?;
        // Hey friend! std::fs::canonicalize() on Windows prepends the \\?\ UNC prefix.
        // This extended-length prefix causes confusion for the AI model and looks ugly in the UI.
        // We strip it if present to keep the workspace root path standard and human-readable.
        #[cfg(windows)]
        {
            let s = root.to_string_lossy();
            if s.starts_with(r"\\?\") {
                root = PathBuf::from(&s[4..]);
            }
        }
        config.root = root;

        // Keep one-level tree traversal by default when callers pass 0.
        if config.tree_depth == 0 {
            config.tree_depth = 1;
        }

        // Avoid forcing every caller to provide an id for quick integrations.
        if config.session_id.trim().is_empty() {
            config.session_id = generate_session_id();
        }

        let tree_dirty = Arc::new(AtomicBool::new(false));
        let agents_md_dirty = Arc::new(AtomicBool::new(false));

        let callback_root = config.root.clone();
        let callback_tree_dirty = Arc::clone(&tree_dirty);
        let callback_agents_dirty = Arc::clone(&agents_md_dirty);

        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            match result {
                Ok(event) => {
                    if event_should_invalidate(&event.kind) {
                        for path in &event.paths {
                            if path_affects_tree(path, &callback_root) {
                                callback_tree_dirty.store(true, Ordering::Relaxed);
                            }
                            if path_affects_agents(path) {
                                callback_agents_dirty.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                }
                Err(_) => {
                    // On watcher backend errors, prefer conservative refresh.
                    callback_tree_dirty.store(true, Ordering::Relaxed);
                    callback_agents_dirty.store(true, Ordering::Relaxed);
                }
            }
        })?;

        watcher.watch(&config.root, RecursiveMode::NonRecursive)?;

        Ok(Self {
            config,
            cached_tree: None,
            cached_agents_md: None,
            tree_dirty,
            agents_md_dirty,
            _watcher: watcher,
        })
    }

    /// Builds a fresh snapshot for the current moment.
    pub fn build(&mut self) -> Result<SessionSnapshot, SnapshotError> {
        let bootstrap =
            blocks::bootstrap::assemble_bootstrap(self.config.role, self.config.session_id.clone());

        if self.cached_agents_md.is_none() || self.agents_md_dirty.swap(false, Ordering::AcqRel) {
            let loaded = blocks::agents_md::read_agents_md(&self.config.root)?;
            self.cached_agents_md = Some(loaded);
        }

        if self.cached_tree.is_none() || self.tree_dirty.swap(false, Ordering::AcqRel) {
            let built = blocks::tree::build_tree(&self.config.root, self.config.tree_depth)?;
            self.cached_tree = Some(built);
        }

        let git = blocks::git::read_git_status(&self.config.root)?;

        let tree = self.cached_tree.clone().unwrap_or_else(|| DirectoryTree {
            root: self.config.root.clone(),
            rendered: String::new(),
        });

        let agents_md = self.cached_agents_md.clone().unwrap_or(None);

        let tool_groups_block = blocks::tool_groups::render_tool_groups(&self.config.tool_groups);

        Ok(SessionSnapshot {
            bootstrap,
            agents_md,
            tree,
            git,
            tool_groups: tool_groups_block,
        })
    }
}

fn event_should_invalidate(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
            | EventKind::Other
    )
}

fn path_affects_tree(path: &Path, root: &Path) -> bool {
    path == root || path.parent() == Some(root)
}

fn path_affects_agents(path: &Path) -> bool {
    path.file_name()
        .map(|name| name == "AGENTS.md")
        .unwrap_or(false)
}

fn generate_session_id() -> String {
    // Hex nanoseconds is enough as a low-dependency per-session discriminator.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_builder_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SnapshotBuilder>();
    }

    #[test]
    fn snapshot_builder_is_not_sync() {
        // SnapshotBuilder must NOT be Sync â€” callers must use Mutex for shared access.
        #[allow(dead_code)]
        fn assert_not_sync<T: ?Sized>()
        where
            T: Sync,
        {
        }
        // This must NOT compile. Leave commented as documentation of intent.
        // assert_not_sync::<SnapshotBuilder>();
    }
}
