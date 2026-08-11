// integration_tests.rs — End-to-End Integration Test Suite for operon-diff
//
// Hey friend! This test suite validates all features of the `operon-diff` crate against real
// temporary Git repositories created using `tempfile` and `libgit2`.

use std::fs::{self, File};
use std::io::Write;
use git2::Repository;
use tempfile::TempDir;

use operon_diff::*;

/// Helper: Initializes a temporary Git repository with configured user signature.
fn setup_test_repo() -> (TempDir, Repository) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let repo = Repository::init(dir.path()).expect("Failed to init git repo");

    let mut config = repo.config().expect("Failed to get repo config");
    config.set_str("user.name", "Operon Tester").unwrap();
    config.set_str("user.email", "tester@operon.dev").unwrap();

    (dir, repo)
}

#[test]
fn test_diff_stats_and_details_empty_and_modified() {
    let (dir, repo) = setup_test_repo();
    let root = dir.path();

    // 1. Initial stats on fresh empty repo
    let stats = get_diff_stats(root).unwrap();
    assert!(stats.has_repo);
    assert_eq!(stats.insertions, 0);
    assert_eq!(stats.deletions, 0);

    // 2. Create an untracked file
    let file_path = root.join("hello.txt");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "Hello world!").unwrap();
    writeln!(file, "Second line").unwrap();
    drop(file);

    // Stats should reflect untracked insertions
    let stats_untracked = get_diff_stats(root).unwrap();
    assert_eq!(stats_untracked.insertions, 2);

    // Details should show 1 unstaged/untracked file
    let details = get_diff_details(root).unwrap();
    assert_eq!(details.unstaged_files.len(), 1);
    assert_eq!(details.unstaged_files[0].path, "hello.txt");
    assert_eq!(details.unstaged_files[0].status, "untracked");

    // 3. Stage the file and commit
    stage_file(root, "hello.txt").unwrap();
    let details_staged = get_diff_details(root).unwrap();
    assert_eq!(details_staged.staged_files.len(), 1);
    assert_eq!(details_staged.unstaged_files.len(), 0);

    let commit_res = commit(&repo, "Initial commit", false).unwrap();
    assert!(!commit_res.oid.is_empty());

    // After commit, workdir is clean
    let stats_clean = get_diff_stats(root).unwrap();
    assert_eq!(stats_clean.insertions, 0);
    assert_eq!(stats_clean.deletions, 0);
}

#[test]
fn test_staging_unstaging_and_reverting() {
    let (dir, repo) = setup_test_repo();
    let root = dir.path();

    // Create and commit initial file
    let file_path = root.join("app.rs");
    fs::write(&file_path, "fn main() {}\n").unwrap();
    stage_file(root, "app.rs").unwrap();
    commit(&repo, "Initial app commit", false).unwrap();

    // Modify tracked file
    fs::write(&file_path, "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();

    // Unstaged modification present
    let stats = get_diff_stats(root).unwrap();
    assert!(stats.insertions > 0);

    // Stage file
    stage_file(root, "app.rs").unwrap();
    let details_staged = get_diff_details(root).unwrap();
    assert_eq!(details_staged.staged_files.len(), 1);

    // Unstage file
    unstage_file(root, "app.rs").unwrap();
    let details_unstaged = get_diff_details(root).unwrap();
    assert_eq!(details_unstaged.unstaged_files.len(), 1);
    assert_eq!(details_unstaged.staged_files.len(), 0);

    // Revert file
    revert_file(root, "app.rs").unwrap();
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content.replace("\r\n", "\n"), "fn main() {}\n");
}

#[test]
fn test_discard_all_including_untracked() {
    let (dir, repo) = setup_test_repo();
    let root = dir.path();

    // Tracked file
    let tracked = root.join("tracked.txt");
    fs::write(&tracked, "version 1\n").unwrap();
    stage_file(root, "tracked.txt").unwrap();
    commit(&repo, "Initial commit", false).unwrap();

    // Edit tracked file and create untracked file + folder
    fs::write(&tracked, "version 2 edited\n").unwrap();
    let untracked_file = root.join("untracked.txt");
    fs::write(&untracked_file, "garbage data\n").unwrap();
    let untracked_dir = root.join("temp_dir");
    fs::create_dir(&untracked_dir).unwrap();
    fs::write(untracked_dir.join("sub.txt"), "sub data\n").unwrap();

    // Discard everything
    discard_all_including_untracked(root).unwrap();

    // Verify tracked file reverted and untracked deleted
    assert_eq!(fs::read_to_string(&tracked).unwrap().replace("\r\n", "\n"), "version 1\n");
    assert!(!untracked_file.exists());
    assert!(!untracked_dir.exists());
}

#[test]
fn test_hunk_staging_and_unstaging() {
    let (dir, repo) = setup_test_repo();
    let root = dir.path();

    let file_path = root.join("data.txt");
    fs::write(&file_path, "line 1\nline 2\nline 3\n").unwrap();
    stage_file(root, "data.txt").unwrap();
    commit(&repo, "Base commit", false).unwrap();

    // Add a new line
    fs::write(&file_path, "line 1\nline 2\nline 3\nline 4 added\n").unwrap();

    let details = get_diff_details(root).unwrap();
    assert_eq!(details.unstaged_files.len(), 1);
    let hunk_header = &details.unstaged_files[0].hunks[0].header;

    // Stage the hunk
    stage_hunk(root, "data.txt", hunk_header).unwrap();

    let details_after_stage = get_diff_details(root).unwrap();
    assert_eq!(details_after_stage.staged_files.len(), 1);

    // Unstage the hunk
    let staged_hunk_header = &details_after_stage.staged_files[0].hunks[0].header;
    unstage_hunk(root, "data.txt", staged_hunk_header).unwrap();

    let details_after_unstage = get_diff_details(root).unwrap();
    assert_eq!(details_after_unstage.staged_files.len(), 0);
    assert_eq!(details_after_unstage.unstaged_files.len(), 1);
}

#[test]
fn test_branch_lifecycle_and_ahead_behind() {
    let (dir, repo) = setup_test_repo();
    let root = dir.path();

    // Create initial commit on main
    let f = root.join("init.txt");
    fs::write(&f, "init").unwrap();
    stage_file(root, "init.txt").unwrap();
    commit(&repo, "Initial commit", false).unwrap();

    // Check current branch
    let current = current_branch(&repo).unwrap();
    assert!(current.is_head);
    assert!(current.name == "main" || current.name == "master");

    // Create feature branch
    let feat_branch = create_branch(&repo, "feature/ui", None).unwrap();
    assert_eq!(feat_branch.name, "feature/ui");

    // List branches
    let branches = list_branches(&repo).unwrap();
    assert!(branches.len() >= 2);

    // Switch to feature branch
    switch_branch(&repo, "feature/ui").unwrap();
    let current_after_switch = current_branch(&repo).unwrap();
    assert_eq!(current_after_switch.name, "feature/ui");

    // Rename branch
    rename_branch(&repo, "feature/ui", "feature/awesome-ui").unwrap();
    let current_renamed = current_branch(&repo).unwrap();
    assert_eq!(current_renamed.name, "feature/awesome-ui");

    // Switch back to main and delete renamed branch
    let main_name = if branches.iter().any(|b| b.name == "main") { "main" } else { "master" };
    switch_branch(&repo, main_name).unwrap();
    delete_branch(&repo, "feature/awesome-ui").unwrap();

    let branches_after_del = list_branches(&repo).unwrap();
    assert!(!branches_after_del.iter().any(|b| b.name == "feature/awesome-ui"));
}

#[test]
fn test_commit_graph_traversal() {
    let (dir, repo) = setup_test_repo();
    let root = dir.path();

    // Create 3 commits
    for i in 1..=3 {
        let file_path = root.join(format!("file_{i}.txt"));
        fs::write(&file_path, format!("content {i}")).unwrap();
        stage_file(root, &format!("file_{i}.txt")).unwrap();
        commit(&repo, &format!("Commit number {i}"), false).unwrap();
    }

    // Retrieve commit graph
    let graph = get_commit_graph(&repo, 10, 0).unwrap();
    assert_eq!(graph.len(), 3);
    assert!(graph[0].is_head); // Topmost topological/time commit is HEAD
    assert_eq!(graph[0].message, "Commit number 3");
    assert_eq!(graph[2].message, "Commit number 1");
}

#[test]
fn test_repo_registry_multi_repo_discovery() {
    let temp_workspace = TempDir::new().unwrap();
    let ws_root = temp_workspace.path();

    // Create two repo subdirectories inside workspace
    let repo1_dir = ws_root.join("repo_alpha");
    let repo2_dir = ws_root.join("repo_beta");

    fs::create_dir_all(&repo1_dir).unwrap();
    fs::create_dir_all(&repo2_dir).unwrap();

    let r1 = Repository::init(&repo1_dir).unwrap();
    let _r2 = Repository::init(&repo2_dir).unwrap();

    let mut config1 = r1.config().unwrap();
    config1.set_str("user.name", "Tester").unwrap();
    config1.set_str("user.email", "test@operon.dev").unwrap();

    let mut registry = RepoRegistry::new();
    let discovered = registry.discover_workspace_repos(ws_root);

    assert_eq!(discovered.len(), 2);

    // Active repo default & switching
    assert!(registry.active_repo().is_some());
    registry.set_active(&repo2_dir).unwrap();
    assert_eq!(registry.active_repo().unwrap().root, repo2_dir);

    registry.remove_repo(&repo1_dir);
    assert_eq!(registry.list_repos().len(), 1);
}

#[tokio::test]
async fn test_async_workspace_wrappers() {
    let (dir, _repo) = setup_test_repo();
    let root = dir.path().to_path_buf();

    // Create file async
    let file_path = root.join("async_test.txt");
    fs::write(&file_path, "async content\n").unwrap();

    stage_file_async(root.clone(), "async_test.txt".to_string()).await.unwrap();

    let commit_res = commit_async(root.clone(), "Async commit".to_string(), false).await.unwrap();
    assert!(!commit_res.oid.is_empty());

    let stats = get_diff_stats_async(root.clone()).await.unwrap();
    assert_eq!(stats.insertions, 0);

    let graph = get_commit_graph_async(root.clone(), 5, 0).await.unwrap();
    assert_eq!(graph.len(), 1);
    assert_eq!(graph[0].message, "Async commit");
}
