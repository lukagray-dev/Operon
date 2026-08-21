use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::error::SnapshotError;
use crate::types::DirectoryTree;

#[derive(Debug, Clone)]
struct TreeEntry {
    relative_path: PathBuf,
    is_dir: bool,
}

/// Builds the gitignore-aware project tree text block.
pub(crate) fn build_tree(root: &Path, tree_depth: usize) -> Result<DirectoryTree, SnapshotError> {
    let mut walker = WalkBuilder::new(root);
    walker.max_depth(Some(tree_depth.saturating_add(1)));

    let mut entries: Vec<TreeEntry> = Vec::new();

    for next in walker.build() {
        let entry = match next {
            Ok(entry) => entry,
            Err(err) => {
                if let Some(io_err) = err.into_io_error() {
                    return Err(io_err.into());
                }
                return Err(std::io::Error::other("directory walk failed").into());
            }
        };

        if entry.depth() == 0 {
            continue;
        }

        let path = entry.path();
        let relative = match path.strip_prefix(root) {
            Ok(rel) => rel.to_path_buf(),
            Err(_) => continue,
        };

        let is_dir = entry
            .file_type()
            .map(|kind| kind.is_dir())
            .unwrap_or_else(|| path.is_dir());

        if should_skip(&relative, is_dir) {
            continue;
        }

        entries.push(TreeEntry {
            relative_path: relative,
            is_dir,
        });
    }

    let rendered = render_hierarchical_tree(entries);

    Ok(DirectoryTree {
        root: root.to_path_buf(),
        rendered,
    })
}

fn render_hierarchical_tree(entries: Vec<TreeEntry>) -> String {
    let mut by_parent: BTreeMap<PathBuf, Vec<TreeEntry>> = BTreeMap::new();

    for entry in entries {
        let parent = entry
            .relative_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        by_parent.entry(parent).or_default().push(entry);
    }

    for children in by_parent.values_mut() {
        children.sort_by(compare_tree_entries);
    }

    let mut output = String::new();
    render_children(Path::new(""), 0, &by_parent, &mut output);
    output
}

fn render_children(
    parent: &Path,
    depth: usize,
    by_parent: &BTreeMap<PathBuf, Vec<TreeEntry>>,
    output: &mut String,
) {
    if let Some(children) = by_parent.get(parent) {
        for child in children {
            output.push_str(&"  ".repeat(depth));
            output.push_str(&entry_name(child));
            output.push('\n');

            if child.is_dir {
                render_children(&child.relative_path, depth + 1, by_parent, output);
            }
        }
    }
}

fn compare_tree_entries(a: &TreeEntry, b: &TreeEntry) -> Ordering {
    match (a.is_dir, b.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => {
            let a_name = leaf_name(&a.relative_path);
            let b_name = leaf_name(&b.relative_path);
            a_name.cmp(&b_name)
        }
    }
}

fn entry_name(entry: &TreeEntry) -> String {
    let mut name = leaf_name(&entry.relative_path).to_string();
    if entry.is_dir {
        name.push('/');
    }
    name
}

fn leaf_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn should_skip(relative_path: &Path, is_dir: bool) -> bool {
    let file_name = relative_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    if is_dir {
        return matches!(
            file_name.as_str(),
            "target" | "node_modules" | ".git" | "dist" | "build" | "__pycache__"
        );
    }

    if file_name.ends_with(".lock") {
        return true;
    }

    matches!(
        file_name.as_str(),
        "Cargo.lock" | "package-lock.json" | "yarn.lock"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_are_sorted_before_files() {
        let mut entries = [
            TreeEntry {
                relative_path: PathBuf::from("b.txt"),
                is_dir: false,
            },
            TreeEntry {
                relative_path: PathBuf::from("a"),
                is_dir: true,
            },
            TreeEntry {
                relative_path: PathBuf::from("c.txt"),
                is_dir: false,
            },
        ];
        entries.sort_by(compare_tree_entries);

        assert_eq!(leaf_name(&entries[0].relative_path), "a");
        assert_eq!(leaf_name(&entries[1].relative_path), "b.txt");
        assert_eq!(leaf_name(&entries[2].relative_path), "c.txt");
    }

    #[test]
    fn lock_and_build_artifacts_are_skipped() {
        assert!(should_skip(Path::new("Cargo.lock"), false));
        assert!(should_skip(Path::new("package-lock.json"), false));
        assert!(should_skip(Path::new("target"), true));
        assert!(!should_skip(Path::new("src"), true));
    }
}
