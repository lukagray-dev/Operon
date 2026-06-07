use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use git2::{Repository, Signature};
use operon_context_snapshot::{Role, SnapshotBuilder, SnapshotConfig};

fn unique_temp_dir(label: &str) -> PathBuf {
    let base = if cfg!(windows) {
        if Path::new("D:\\").exists() {
            PathBuf::from(r"D:\tmp")
        } else {
            PathBuf::from(r"C:\tmp")
        }
    } else {
        std::env::temp_dir()
    };
    fs::create_dir_all(&base).expect("create test base dir");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    base.join(format!("operon_context_snapshot_{label}_{nonce}"))
}

fn create_clean_temp_dir(label: &str) -> PathBuf {
    let root = unique_temp_dir(label);
    fs::create_dir_all(&root).expect("create temp root");
    root
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

fn make_builder(root: &Path, tree_depth: usize) -> SnapshotBuilder {
    SnapshotBuilder::new(SnapshotConfig {
        root: root.to_path_buf(),
        role: Role::Owner,
        session_id: "test-session".to_string(),
        tree_depth,
        tool_groups: Vec::new(),
    })
    .expect("builder")
}

#[test]
fn snapshot_renders_all_non_git_sections() {
    let root = create_clean_temp_dir("render_non_git");
    write_file(&root.join("AGENTS.md"), "Follow local rules.\n");
    write_file(&root.join("src/lib.rs"), "pub fn ok() {}\n");
    write_file(&root.join("README.md"), "hello\n");

    let mut builder = make_builder(&root, 1);
    let snapshot = builder.build().expect("build snapshot");
    let rendered = snapshot.render();

    assert!(rendered.contains("=== OPERON SESSION ==="));
    assert!(rendered.contains("Role: Owner"));
    assert!(rendered.contains("=== INSTRUCTIONS ==="));
    assert!(rendered.contains("Follow local rules."));
    assert!(rendered.contains("=== PROJECT ==="));
    assert!(rendered.contains("Root: "));
    assert!(rendered.contains("src/"));
    assert!(rendered.contains("README.md"));
    assert!(!rendered.contains("=== GIT ==="));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn snapshot_uses_none_when_agents_missing() {
    let root = create_clean_temp_dir("agents_none");
    write_file(&root.join("src/lib.rs"), "pub fn ok() {}\n");

    let mut builder = make_builder(&root, 1);
    let snapshot = builder.build().expect("build snapshot");
    let rendered = snapshot.render();

    assert!(rendered.contains("=== INSTRUCTIONS ===\n(none)\n"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn tree_filters_lockfiles_and_common_build_dirs() {
    let root = create_clean_temp_dir("tree_filter");

    write_file(&root.join("src/lib.rs"), "pub fn ok() {}\n");
    write_file(&root.join("Cargo.lock"), "ignored\n");
    write_file(&root.join("package-lock.json"), "ignored\n");
    write_file(&root.join("yarn.lock"), "ignored\n");
    write_file(&root.join("foo.lock"), "ignored\n");
    write_file(&root.join("target/output.txt"), "ignored\n");
    write_file(&root.join("node_modules/pkg/index.js"), "ignored\n");
    write_file(&root.join("dist/build.txt"), "ignored\n");
    write_file(&root.join("build/log.txt"), "ignored\n");
    write_file(&root.join("__pycache__/a.pyc"), "ignored\n");

    let mut builder = make_builder(&root, 2);
    let snapshot = builder.build().expect("build snapshot");
    let rendered_tree = snapshot.tree.rendered;

    assert!(rendered_tree.contains("src/"));
    assert!(!rendered_tree.contains("Cargo.lock"));
    assert!(!rendered_tree.contains("package-lock.json"));
    assert!(!rendered_tree.contains("yarn.lock"));
    assert!(!rendered_tree.contains("foo.lock"));
    assert!(!rendered_tree.contains("target/"));
    assert!(!rendered_tree.contains("node_modules/"));
    assert!(!rendered_tree.contains("dist/"));
    assert!(!rendered_tree.contains("build/"));
    assert!(!rendered_tree.contains("__pycache__/"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn git_status_is_present_inside_git_repo() {
    let root = create_clean_temp_dir("git_status");

    let repo = Repository::init(&root).expect("init repo");
    write_file(&root.join("tracked.txt"), "line-1\n");

    let mut index = repo.index().expect("index");
    index
        .add_path(Path::new("tracked.txt"))
        .expect("add tracked");
    let tree_id = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_id).expect("find tree");
    let sig = Signature::now("Operon", "operon@example.com").expect("signature");
    repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
        .expect("commit");

    write_file(&root.join("tracked.txt"), "line-1\nline-2\n");
    write_file(&root.join("staged.txt"), "staged\n");
    write_file(&root.join("untracked.txt"), "new\n");

    let mut index = repo.index().expect("index");
    index.add_path(Path::new("staged.txt")).expect("add staged");
    index.write().expect("write index");

    let mut builder = make_builder(&root, 1);
    let snapshot = builder.build().expect("build snapshot");
    let git = snapshot.git.as_ref().expect("git block");

    assert!(!git.branch.is_empty());
    assert!(git.staged >= 1);
    assert!(git.unstaged >= 1);
    assert!(git.untracked >= 1);
    assert!(git.insertions + git.deletions >= 1);

    let rendered = snapshot.render();
    assert!(rendered.contains("=== GIT ==="));
    assert!(rendered.contains("Branch: "));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn watcher_invalidation_refreshes_agents_and_tree() {
    let root = create_clean_temp_dir("watcher_refresh");

    write_file(&root.join("AGENTS.md"), "v1\n");
    write_file(&root.join("alpha.txt"), "a\n");

    let mut builder = make_builder(&root, 1);
    let initial = builder.build().expect("first build");
    assert_eq!(initial.agents_md.as_deref(), Some("v1\n"));
    assert!(initial.tree.rendered.contains("alpha.txt"));

    write_file(&root.join("AGENTS.md"), "v2\n");
    write_file(&root.join("beta.txt"), "b\n");

    let mut observed = false;
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));
        let snapshot = builder.build().expect("rebuild");
        let agents_ok = snapshot.agents_md.as_deref() == Some("v2\n");
        let tree_ok = snapshot.tree.rendered.contains("beta.txt");
        if agents_ok && tree_ok {
            observed = true;
            break;
        }
    }

    assert!(
        observed,
        "watcher did not invalidate AGENTS.md + tree cache in time"
    );

    let _ = fs::remove_dir_all(root);
}
