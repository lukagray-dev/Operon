# operon-diff

**Production-grade Git integration engine with async UI interop for Operon**

`operon-diff` provides comprehensive Git repository management via `libgit2`, including diff parsing, staging operations, branch management, commit creation, visual commit graphs, remote synchronization, and multi-repository workspace tracking. All blocking operations have non-blocking async wrappers for seamless UI integration.

---

## Overview

This crate is the **core Git integration layer** for Operon, handling all version control operations from the desktop UI (Slint) and future web/CLI clients. It wraps `libgit2` with ergonomic, production-ready APIs and rich DTOs for frontend consumption.

```mermaid
flowchart TB
    UI[Slint Desktop UI / Web Frontend] --> Async[workspace.rs<br/>Async Wrappers]
    Async --> Core[Core Operations]
    
    subgraph Core Operations
        Status[status.rs<br/>Repo Discovery<br/>Diff Stats]
        Stage[stage.rs<br/>Stage/Unstage<br/>Revert/Discard<br/>Hunk Patches]
        Commit[commit.rs<br/>Commit Creation<br/>Amend Support]
        Branch[branch.rs<br/>List/Create/Switch<br/>Delete/Rename]
        Graph[graph.rs<br/>Commit Graph<br/>Unpushed Detection]
        Remote[remote.rs<br/>Push/Fetch/Pull<br/>Auth Management]
        Registry[repo_manager.rs<br/>Multi-Repo Tracking]
    end
    
    Core --> Diff[diff.rs<br/>Patch Parser]
    Core --> Git[libgit2<br/>Low-level Git API]
    
    Diff --> DTO[dto.rs<br/>Serde DTOs]
    
    style UI fill:#87CEEB
    style Async fill:#FFD700
    style DTO fill:#90EE90
    style Git fill:#FF6B6B
```

**Key Features**:
- ✅ **File-level and hunk-level staging** with unified patch generation
- ✅ **Visual commit graph** with branch tags and unpushed commit detection
- ✅ **Branch operations** (create, switch, delete, rename) with upstream tracking
- ✅ **Remote operations** (push, fetch, fast-forward pull) with auto-credential handling
- ✅ **Multi-repository workspaces** with in-memory registry
- ✅ **Async wrappers** for all blocking operations (`tokio::task::spawn_blocking`)
- ✅ **Rich DTOs** with camelCase serialization for frontend interop

---

## Architecture

### Module Organization

```mermaid
graph TD
    A[operon-diff] --> B[status.rs<br/>Repository Discovery]
    A --> C[stage.rs<br/>Index Manipulation]
    A --> D[commit.rs<br/>Commit Creation]
    A --> E[branch.rs<br/>Branch Management]
    A --> F[graph.rs<br/>Visual History]
    A --> G[remote.rs<br/>Network Operations]
    A --> H[repo_manager.rs<br/>Multi-Repo Registry]
    A --> I[diff.rs<br/>Patch Parser]
    A --> J[workspace.rs<br/>Async Wrappers]
    A --> K[dto.rs<br/>Data Transfer Objects]
    A --> L[error.rs<br/>Error Types]
    
    B --> M[libgit2]
    C --> M
    D --> M
    E --> M
    F --> M
    G --> M[libgit2 + auth-git2]
    H --> M
    I --> M
    
    style A fill:#87CEEB
    style J fill:#FFD700
    style K fill:#90EE90
    style M fill:#FF6B6B
```

---

## Core Operations

### 1. Repository Discovery & Diff Statistics

**Module**: `status.rs`

```mermaid
flowchart TD
    A[discover_repository path] --> B{.git exists?}
    B -->|No| C[Search parent dirs]
    B -->|Yes| D[Open Repository]
    C --> E{Found?}
    E -->|Yes| D
    E -->|No| F[NoRepository error]
    D --> G[Return Repository handle]
    
    style D fill:#90EE90
    style F fill:#FF6B6B
```

**API**:

```rust
// Discover repository root from workspace path
pub fn discover_repository<P: AsRef<Path>>(workspace_root: P) 
    -> Result<Repository, DiffError>;

// Quick stats for header badge (+X -Y)
pub fn get_diff_stats<P: AsRef<Path>>(workspace_root: P) 
    -> Result<GitDiffStats, DiffError>;

// Full diff tree with file-level hunks
pub fn get_diff_details<P: AsRef<Path>>(workspace_root: P) 
    -> Result<RepositoryDiff, DiffError>;
```

**Diff Computation**:

```mermaid
flowchart LR
    A[Repository] --> B[Unstaged:<br/>Index → Workdir]
    A --> C[Staged:<br/>HEAD → Index]
    
    B --> D[parse_diff]
    C --> D
    
    D --> E[Vec FileDiff]
    E --> F[RepositoryDiff<br/>unstaged_files<br/>staged_files]
    
    style B fill:#FFD700
    style C fill:#87CEEB
    style F fill:#90EE90
```

**Stats Calculation**:
- **Staged**: `diff_tree_to_index(HEAD, Index)` (handles unborn HEAD)
- **Unstaged**: `diff_index_to_workdir(Index, Workdir)` with `include_untracked(true)`
- **Total**: Sum of staged and unstaged insertions/deletions

---

### 2. Staging Operations

**Module**: `stage.rs`

#### File-Level Operations

```mermaid
flowchart TD
    A[stage_file] --> B[index.add_path]
    B --> C[index.write]
    
    D[unstage_file] --> E{HEAD exists?}
    E -->|Yes| F[reset_default to HEAD]
    E -->|No| G[index.remove_path]
    
    H[revert_file] --> I{Untracked?}
    I -->|Yes| J[Delete from filesystem]
    I -->|No| K[checkout_index force]
    
    L[stage_all_files] --> M[index.add_all]
    N[revert_all_files] --> O[checkout_index force]
    P[discard_all_including_untracked] --> O
    P --> Q[Remove untracked files]
    
    style C fill:#90EE90
    style J fill:#FF6B6B
    style K fill:#FFD700
```

**API**:

```rust
// Stage single file
pub fn stage_file<P: AsRef<Path>>(workspace_root: P, relative_path: &str) 
    -> Result<(), DiffError>;

// Unstage file (reset to HEAD)
pub fn unstage_file<P: AsRef<Path>>(workspace_root: P, relative_path: &str) 
    -> Result<(), DiffError>;

// Revert unstaged changes (untracked files deleted)
pub fn revert_file<P: AsRef<Path>>(workspace_root: P, relative_path: &str) 
    -> Result<(), DiffError>;

// Stage all modifications and untracked files
pub fn stage_all_files<P: AsRef<Path>>(workspace_root: P) 
    -> Result<(), DiffError>;

// Discard all unstaged changes (tracked files only)
pub fn revert_all_files<P: AsRef<Path>>(workspace_root: P) 
    -> Result<(), DiffError>;

// Nuclear option: discard + delete untracked
pub fn discard_all_including_untracked<P: AsRef<Path>>(workspace_root: P) 
    -> Result<(), DiffError>;
```

#### Hunk-Level Operations

**Unified Patch Generation** for selective staging:

```mermaid
flowchart TD
    A[stage_hunk file, header] --> B[Get workdir diff<br/>for target file]
    B --> C[parse_diff]
    C --> D{Find matching hunk<br/>by header?}
    D -->|Not found| E[Error]
    D -->|Found| F[build_unified_patch]
    F --> G[Diff::from_buffer]
    G --> H[repo.apply Index]
    H --> I[Success]
    
    style F fill:#FFD700
    style H fill:#90EE90
    style E fill:#FF6B6B
```

**Patch Format** (example):

```diff
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -245,6 +245,21 @@ fn process_data(input: &str) -> Result<String> {
+    // New validation logic
+    if input.is_empty() {
+        return Err("Empty input");
+    }
     let result = transform(input);
```

**API**:

```rust
// Stage single hunk within file
pub fn stage_hunk<P: AsRef<Path>>(
    workspace_root: P,
    relative_path: &str,
    hunk_header: &str,
) -> Result<(), DiffError>;

// Unstage single hunk (reverse patch)
pub fn unstage_hunk<P: AsRef<Path>>(
    workspace_root: P,
    relative_path: &str,
    hunk_header: &str,
) -> Result<(), DiffError>;
```

**Reverse Patch** for unstaging:
- Swap `+` and `-` line types
- Invert hunk header ranges: `@@ -new_start,new_lines +old_start,old_lines @@`
- Handle file status inversions (added → deleted, deleted → added)

---

### 3. Commit Creation

**Module**: `commit.rs`

```mermaid
flowchart TD
    A[commit message, amend] --> B[Resolve signature<br/>from git config]
    B -->|Missing| C[SignatureMissing error]
    B -->|Found| D[Write index to tree]
    D --> E{amend?}
    E -->|Yes| F[parent.amend]
    E -->|No| G{HEAD exists?}
    G -->|Yes| H[repo.commit with parent]
    G -->|No| I[repo.commit without parent<br/>Initial commit]
    F --> J[CommitResult]
    H --> J
    I --> J
    
    style C fill:#FF6B6B
    style J fill:#90EE90
```

**API**:

```rust
pub fn commit(
    repo: &Repository,
    message: &str,
    amend: bool,
) -> Result<CommitResult, DiffError>;

pub fn commit_workspace<P: AsRef<Path>>(
    workspace_root: P,
    message: &str,
    amend: bool,
) -> Result<CommitResult, DiffError>;
```

**Signature Resolution**:
- Reads `user.name` and `user.email` from:
  1. Repository-local `.git/config`
  2. Global `~/.gitconfig`
  3. Environment variables (`GIT_AUTHOR_NAME`, `GIT_AUTHOR_EMAIL`)
- Returns `DiffError::SignatureMissing` if unconfigured

**Amend Support**:
- Modifies last commit in-place
- Preserves parent relationships
- Updates message and tree
- **Warning**: Only amend unpushed commits

---

### 4. Branch Management

**Module**: `branch.rs`

```mermaid
flowchart LR
    A[Branch Operations] --> B[current_branch]
    A --> C[list_branches]
    A --> D[create_branch]
    A --> E[switch_branch]
    A --> F[delete_branch]
    A --> G[rename_branch]
    
    B --> H[BranchInfo]
    C --> I[Vec BranchInfo]
    D --> H
    
    H --> J[name, is_head<br/>upstream<br/>ahead, behind]
    
    style H fill:#90EE90
    style I fill:#90EE90
```

**API**:

```rust
// Get current HEAD branch with tracking info
pub fn current_branch(repo: &Repository) 
    -> Result<BranchInfo, DiffError>;

// List all local branches with ahead/behind metrics
pub fn list_branches(repo: &Repository) 
    -> Result<Vec<BranchInfo>, DiffError>;

// Create new branch at commit (or HEAD)
pub fn create_branch(
    repo: &Repository,
    name: &str,
    target_commit_sha: Option<&str>,
) -> Result<BranchInfo, DiffError>;

// Switch to branch (checkout)
pub fn switch_branch(repo: &Repository, name: &str) 
    -> Result<(), DiffError>;

// Delete local branch
pub fn delete_branch(repo: &Repository, name: &str) 
    -> Result<(), DiffError>;

// Rename existing branch
pub fn rename_branch(
    repo: &Repository,
    old_name: &str,
    new_name: &str,
) -> Result<(), DiffError>;
```

**BranchInfo Structure**:

```rust
pub struct BranchInfo {
    pub name: String,              // "main"
    pub is_head: bool,             // Currently checked out?
    pub upstream: Option<String>,  // "origin/main"
    pub ahead: usize,              // Commits ahead of upstream
    pub behind: usize,             // Commits behind upstream
}
```

**Upstream Tracking**:

```mermaid
flowchart TD
    A[Local Branch] --> B{Has upstream?}
    B -->|No| C[ahead = 0<br/>behind = 0]
    B -->|Yes| D[graph_ahead_behind<br/>local_oid, upstream_oid]
    D --> E[ahead: local..upstream<br/>behind: upstream..local]
    
    style E fill:#90EE90
```

**Unborn Branch Handling**:
- Fresh repos with no commits: `HEAD` → `refs/heads/main`
- Returns `BranchInfo` with `name = "main"`, `is_head = true`, no upstream

---

### 5. Visual Commit Graph

**Module**: `graph.rs`

```mermaid
flowchart TD
    A[get_commit_graph<br/>limit, skip] --> B[Resolve HEAD oid]
    B --> C[Build branch_tag_map<br/>Oid → branch names]
    C --> D[Compute unpushed_set<br/>Local-only commits]
    D --> E[Initialize revwalk<br/>TOPOLOGICAL + TIME sort]
    E --> F[Push refs/heads/*]
    F --> G[Paginate:<br/>skip N, take limit]
    G --> H[For each commit OID]
    H --> I[Extract:<br/>hash, message, author]
    I --> J[Lookup branch_tag<br/>from map]
    J --> K[Check is_head<br/>Check is_local]
    K --> L[GitGraphCommit]
    L --> M[Vec GitGraphCommit]
    
    style M fill:#90EE90
```

**API**:

```rust
// Paginated commit history
pub fn get_commit_graph(
    repo: &Repository,
    limit: usize,  // Max commits to return (0 = all)
    skip: usize,   // Pagination offset
) -> Result<Vec<GitGraphCommit>, DiffError>;

// Resolve branch tag for specific commit
pub fn branch_tag_for(repo: &Repository, oid: Oid) -> String;
```

**GitGraphCommit Structure**:

```rust
pub struct GitGraphCommit {
    pub hash: String,         // Full 40-char SHA
    pub short_hash: String,   // 7-char abbreviated
    pub message: String,      // First line summary
    pub author: String,       // Name or email
    pub branch_tag: String,   // "main, develop" (comma-separated)
    pub is_head: bool,        // Current HEAD commit?
    pub is_local: bool,       // Unpushed to upstream?
}
```

**Branch Tag Map**:

```mermaid
flowchart LR
    A[Iterate local branches] --> B[branch.get.target]
    B --> C[Map: Oid → Vec branch_name]
    C --> D[Join with comma:<br/>Oid → name1, name2]
    
    style D fill:#90EE90
```

**Unpushed Detection**:

```mermaid
flowchart TD
    A[Current branch] --> B{Has upstream?}
    B -->|No| C[Compare with all<br/>remote tracking branches]
    B -->|Yes| D[revwalk.push local_oid<br/>revwalk.hide upstream_oid]
    C --> E[revwalk.push local_oid<br/>hide all remote/* tips]
    D --> F[Collect reachable OIDs<br/>= unpushed commits]
    E --> F
    
    style F fill:#90EE90
```

**Sorting**: `Sort::TOPOLOGICAL | Sort::TIME`
- Topological: Parents before children
- Time: Recent commits first

---

### 6. Remote Operations

**Module**: `remote.rs`

```mermaid
flowchart TD
    A[Remote Operations] --> B[push]
    A --> C[fetch]
    A --> D[pull]
    
    B --> E[auth-git2<br/>Credential resolution]
    C --> E
    D --> E
    
    E --> F[SSH Agent]
    E --> G[SSH Keys ~/.ssh/id_*]
    E --> H[HTTPS Token]
    
    D --> I[Fetch + Merge Analysis]
    I --> J{Fast-forward?}
    J -->|Yes| K[checkout + update HEAD]
    J -->|No| L[MergeConflict error]
    
    style K fill:#90EE90
    style L fill:#FF6B6B
```

**API**:

```rust
// Push local branch to remote
pub fn push(
    repo: &Repository,
    remote_name: &str,
    branch: &str,
) -> Result<(), DiffError>;

// Fetch refs and objects from remote
pub fn fetch(
    repo: &Repository,
    remote_name: &str,
) -> Result<(), DiffError>;

// Pull with fast-forward-only merge
pub fn pull(
    repo: &Repository,
    remote_name: &str,
    branch: &str,
) -> Result<(), DiffError>;
```

**Authentication** (`auth-git2`):

| Method | Priority | Fallback |
|--------|----------|----------|
| **SSH Agent** | 1 | Standard on Linux/macOS |
| **SSH Keys** | 2 | `~/.ssh/id_rsa`, `~/.ssh/id_ed25519` |
| **HTTPS Token** | 3 | `credential.helper` from git config |

**Pull Strategy**:

```mermaid
sequenceDiagram
    participant Caller
    participant pull
    participant fetch
    participant merge_analysis
    participant checkout
    
    Caller->>pull: pull(remote, branch)
    pull->>fetch: Fetch remote changes
    fetch-->>pull: Success
    pull->>merge_analysis: Analyze local vs remote
    merge_analysis-->>pull: Analysis result
    
    alt Up to date
        pull-->>Caller: Success (no changes)
    else Fast-forward
        pull->>checkout: Checkout remote tree
        checkout-->>pull: Done
        pull->>pull: Update HEAD reference
        pull-->>Caller: Success
    else Non-fast-forward
        pull-->>Caller: MergeConflict error
    end
```

**Error Handling**:
- `RemoteAuth`: Credential failure, network timeout
- `MergeConflict`: Non-fast-forwardable pull
- `BranchNotFound`: Remote tracking branch missing

---

### 7. Multi-Repository Workspace

**Module**: `repo_manager.rs`

```mermaid
flowchart TD
    A[RepoRegistry] --> B[discover_workspace_repos]
    A --> C[add_repo]
    A --> D[remove_repo]
    A --> E[set_active]
    A --> F[active_repo]
    A --> G[list_repos]
    
    B --> H{Scan workspace root}
    H --> I[Check .git in root]
    H --> J[Check .git in subdirs]
    I --> K[Filter valid repos]
    J --> K
    K --> L[Vec RepoEntry]
    
    style L fill:#90EE90
```

**API**:

```rust
pub struct RepoRegistry {
    repos: Vec<RepoEntry>,
    active_root: Option<PathBuf>,
}

impl RepoRegistry {
    // Create empty registry
    pub fn new() -> Self;
    
    // Discover repos in workspace (non-recursive)
    pub fn discover_workspace_repos<P: AsRef<Path>>(
        &mut self,
        workspace_root: P,
    ) -> Vec<RepoEntry>;
    
    // Manually add repository
    pub fn add_repo<P: AsRef<Path>>(
        &mut self,
        root: P,
    ) -> Result<RepoEntry, DiffError>;
    
    // Remove repository from tracking
    pub fn remove_repo<P: AsRef<Path>>(&mut self, root: P) -> bool;
    
    // Switch active repository
    pub fn set_active<P: AsRef<Path>>(&mut self, root: P) 
        -> Result<(), DiffError>;
    
    // Get current active repo
    pub fn active_repo(&self) -> Option<&RepoEntry>;
    
    // List all tracked repos
    pub fn list_repos(&self) -> Vec<RepoEntry>;
}
```

**RepoEntry Structure**:

```rust
pub struct RepoEntry {
    pub root: PathBuf,        // Absolute path to .git parent
    pub name: String,         // Directory name
    pub is_active: bool,      // Currently selected?
    pub has_changes: bool,    // Uncommitted modifications?
}
```

**Discovery Strategy**:

```mermaid
flowchart TD
    A[workspace_root] --> B{.git exists?}
    B -->|Yes| C[Add to candidates]
    B -->|No| D[Skip]
    C --> E[Scan subdirectories]
    D --> E
    E --> F{subdir/.git exists?}
    F -->|Yes| G[Add to candidates]
    F -->|No| H[Skip subdir]
    G --> I[Filter: discover_repository OK?]
    H --> I
    I --> J[Calculate has_changes]
    J --> K[Vec RepoEntry]
    
    style K fill:#90EE90
```

**Non-Recursive**: Only scans workspace root + immediate subdirectories (not nested)

**Active Repository**:
- First discovered repo is active by default
- UI operations target active repo
- `set_active()` updates `is_active` flags

---

### 8. Diff Parsing

**Module**: `diff.rs`

```mermaid
flowchart TD
    A[git2::Diff] --> B[Iterate deltas]
    B --> C[Patch::from_diff idx]
    C --> D[Extract file path<br/>file_name, dir_path]
    D --> E[Determine status<br/>added/deleted/modified]
    E --> F[Get line_stats<br/>insertions, deletions]
    F --> G[Iterate hunks]
    G --> H[Extract hunk header<br/>old_start, new_start<br/>old_lines, new_lines]
    H --> I[Iterate lines in hunk]
    I --> J[Extract line_type<br/>origin: +, -, space]
    J --> K[Build DiffLine]
    K --> L[Build DiffHunk]
    L --> M[Build FileDiff]
    M --> N[Vec FileDiff]
    
    style N fill:#90EE90
```

**Data Structures**:

```rust
pub struct FileDiff {
    pub path: String,           // "gui/ui/app.slint"
    pub file_name: String,      // "app.slint"
    pub dir_path: String,       // "gui/ui"
    pub status: String,         // "modified"
    pub insertions: usize,      // Lines added
    pub deletions: usize,       // Lines removed
    pub hunks: Vec<DiffHunk>,   // Modification hunks
    pub is_expanded: bool,      // UI accordion state
}

pub struct DiffHunk {
    pub header: String,         // "@@ -245,6 +245,21 @@"
    pub lines: Vec<DiffLine>,   // Hunk content
    pub old_start: u32,         // Line in old file
    pub old_lines: u32,         // Lines in old
    pub new_start: u32,         // Line in new file
    pub new_lines: u32,         // Lines in new
}

pub struct DiffLine {
    pub line_type: char,        // '+', '-', ' '
    pub content: String,        // Line text
    pub old_line_num: Option<u32>,
    pub new_line_num: Option<u32>,
}
```

**Status Mapping**:

| git2::Delta | status string |
|-------------|---------------|
| `Added` | `"added"` |
| `Deleted` | `"deleted"` |
| `Modified` | `"modified"` |
| `Renamed` | `"renamed"` |
| `Typechange` | `"typechanged"` |
| `Untracked` | `"untracked"` |

---

### 9. Async Wrappers

**Module**: `workspace.rs`

**Purpose**: Prevent UI thread blocking by wrapping all synchronous Git operations in `tokio::task::spawn_blocking`

```mermaid
sequenceDiagram
    participant UI as Slint UI Thread
    participant Async as workspace.rs
    participant Tokio as Tokio Runtime
    participant Sync as Sync Operation
    
    UI->>Async: get_diff_stats_async(path)
    Async->>Tokio: spawn_blocking(move || ...)
    Tokio->>Sync: status::get_diff_stats(path)
    Sync-->>Tokio: Result GitDiffStats
    Tokio-->>Async: JoinHandle result
    Async-->>UI: Result GitDiffStats
    
    Note over UI,Async: Non-blocking await
    Note over Tokio,Sync: Runs on thread pool
```

**Naming Convention**: Every sync operation has an `_async` variant

**Example**:

```rust
// Synchronous (blocks thread)
pub fn get_diff_stats<P: AsRef<Path>>(workspace_root: P) 
    -> Result<GitDiffStats, DiffError>;

// Async wrapper (non-blocking)
pub async fn get_diff_stats_async(workspace_root: PathBuf) 
    -> Result<GitDiffStats, DiffError> {
    task::spawn_blocking(move || status::get_diff_stats(workspace_root))
        .await?
}
```

**UI Integration**:

```rust
// ❌ DON'T: Blocks UI thread, causes frame drops
let stats = operon_diff::get_diff_stats(workspace)?;

// ✅ DO: Non-blocking, UI stays responsive
let stats = operon_diff::get_diff_stats_async(workspace).await?;
```

**Complete API Surface**:

| Sync Operation | Async Wrapper |
|----------------|---------------|
| `get_diff_stats` | `get_diff_stats_async` |
| `get_diff_details` | `get_diff_details_async` |
| `stage_file` | `stage_file_async` |
| `unstage_file` | `unstage_file_async` |
| `revert_file` | `revert_file_async` |
| `stage_all_files` | `stage_all_files_async` |
| `revert_all_files` | `revert_all_files_async` |
| `discard_all_including_untracked` | `discard_all_including_untracked_async` |
| `stage_hunk` | `stage_hunk_async` |
| `unstage_hunk` | `unstage_hunk_async` |
| `commit_workspace` | `commit_async` |
| `current_branch_workspace` | `current_branch_async` |
| `list_branches_workspace` | `list_branches_async` |
| `create_branch_workspace` | `create_branch_async` |
| `switch_branch_workspace` | `switch_branch_async` |
| `delete_branch_workspace` | `delete_branch_async` |
| `rename_branch_workspace` | `rename_branch_async` |
| `get_commit_graph_workspace` | `get_commit_graph_async` |
| `push_workspace` | `push_async` |
| `fetch_workspace` | `fetch_async` |
| `pull_workspace` | `pull_async` |
| `discover_workspace_repos` | `discover_workspace_repos_async` |

---

## Data Transfer Objects (DTOs)

**Module**: `dto.rs`

All DTOs use `#[serde(rename_all = "camelCase")]` for frontend interop:

```rust
// Rust: snake_case fields
pub struct BranchInfo {
    pub is_head: bool,
    pub upstream: Option<String>,
}

// JSON/Slint: camelCase properties
{
  "isHead": true,
  "upstream": "origin/main"
}
```

**Complete DTO Catalog**:

| DTO | Purpose | Key Fields |
|-----|---------|-----------|
| `GitDiffStats` | Header badge counts | `has_repo`, `insertions`, `deletions` |
| `RepositoryDiff` | Full diff tree | `staged_files`, `unstaged_files`, `total_insertions` |
| `FileDiff` | Single file changes | `path`, `status`, `hunks`, `is_expanded` |
| `DiffHunk` | Modification block | `header`, `lines`, `old_start`, `new_start` |
| `DiffLine` | Individual line change | `line_type`, `content`, `old_line_num`, `new_line_num` |
| `BranchInfo` | Branch metadata | `name`, `is_head`, `upstream`, `ahead`, `behind` |
| `GitGraphCommit` | Visual commit node | `hash`, `message`, `author`, `branch_tag`, `is_local` |
| `CommitResult` | Commit creation result | `oid` |
| `RepoEntry` | Repository registry entry | `root`, `name`, `is_active`, `has_changes` |

---

## Error Handling

**Module**: `error.rs`

```mermaid
flowchart TD
    A[Operation] --> B{Error Type}
    
    B -->|libgit2 failure| C[DiffError::Git]
    B -->|File I/O| D[DiffError::Io]
    B -->|No repo found| E[DiffError::NoRepository]
    B -->|HEAD invalid| F[DiffError::HeadResolution]
    B -->|Tokio join| G[DiffError::TaskJoin]
    B -->|Missing signature| H[DiffError::SignatureMissing]
    B -->|Repo not tracked| I[DiffError::RepoNotFound]
    B -->|Branch missing| J[DiffError::BranchNotFound]
    B -->|Auth failure| K[DiffError::RemoteAuth]
    B -->|Non-FF merge| L[DiffError::MergeConflict]
    
    C --> M[Display to user]
    D --> M
    E --> M
    F --> M
    G --> M
    H --> N[Prompt: Configure git]
    I --> M
    J --> M
    K --> O[Prompt: Check credentials]
    L --> P[Suggest: Merge/rebase]
    
    style N fill:#FFD700
    style O fill:#FFD700
    style P fill:#FFD700
    style M fill:#90EE90
```

**Error Variants**:

```rust
#[derive(Debug, Error)]
pub enum DiffError {
    #[error("Git libgit2 error: {0}")]
    Git(#[from] git2::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("No git repository found: {0}")]
    NoRepository(String),
    
    #[error("HEAD commit resolution failed: {0}")]
    HeadResolution(String),
    
    #[error("Async task execution error: {0}")]
    TaskJoin(String),
    
    #[error("Git signature missing: {0}")]
    SignatureMissing(String),
    
    #[error("Repository not found: {0}")]
    RepoNotFound(String),
    
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
    
    #[error("Remote authentication failed: {0}")]
    RemoteAuth(String),
    
    #[error("Merge conflict encountered: {0}")]
    MergeConflict(String),
}
```

**UI Integration**:

```rust
match result {
    Ok(data) => show_success(data),
    Err(DiffError::SignatureMissing(msg)) => {
        show_dialog("Git Configuration Required", msg);
        suggest_command("git config --global user.name \"Your Name\"");
    }
    Err(DiffError::RemoteAuth(msg)) => {
        show_dialog("Authentication Failed", msg);
        suggest_ssh_setup();
    }
    Err(e) => show_error(e.to_string()),
}
```

---

## Usage Examples

### Basic Workflow

```rust
use operon_diff::*;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), DiffError> {
    let workspace = PathBuf::from("D:/Operon");
    
    // 1. Get diff stats
    let stats = get_diff_stats_async(workspace.clone()).await?;
    println!("Changes: +{} -{}", stats.insertions, stats.deletions);
    
    // 2. Get detailed diff
    let diff = get_diff_details_async(workspace.clone()).await?;
    for file in &diff.unstaged_files {
        println!("{}: {} ({})", 
            file.status, 
            file.path, 
            file.hunks.len()
        );
    }
    
    // 3. Stage specific file
    stage_file_async(
        workspace.clone(),
        "src/main.rs".to_string(),
    ).await?;
    
    // 4. Commit changes
    let result = commit_async(
        workspace.clone(),
        "feat: Add new feature".to_string(),
        false,
    ).await?;
    println!("Committed: {}", result.oid);
    
    // 5. Push to remote
    push_async(
        workspace,
        "origin".to_string(),
        "main".to_string(),
    ).await?;
    
    Ok(())
}
```

### Hunk-Level Staging

```rust
// Get diff with hunks
let diff = get_diff_details_async(workspace.clone()).await?;

// Find target file
let file = diff.unstaged_files
    .iter()
    .find(|f| f.path == "src/parser.rs")
    .unwrap();

// Stage second hunk only
let hunk_header = &file.hunks[1].header;
stage_hunk_async(
    workspace,
    file.path.clone(),
    hunk_header.clone(),
).await?;
```

### Branch Management

```rust
// List branches with tracking
let branches = list_branches_async(workspace.clone()).await?;
for branch in &branches {
    let tracking = if let Some(upstream) = &branch.upstream {
        format!("↑{} ↓{} tracking {}", 
            branch.ahead, 
            branch.behind, 
            upstream
        )
    } else {
        "no upstream".to_string()
    };
    
    let marker = if branch.is_head { "*" } else { " " };
    println!("{} {} ({})", marker, branch.name, tracking);
}

// Create and switch to new branch
create_branch_async(
    workspace.clone(),
    "feat/new-feature".to_string(),
    None,
).await?;

switch_branch_async(
    workspace,
    "feat/new-feature".to_string(),
).await?;
```

### Commit Graph Visualization

```rust
// Get first 20 commits
let commits = get_commit_graph_async(
    workspace,
    20,  // limit
    0,   // skip
).await?;

for commit in commits {
    let head_marker = if commit.is_head { "HEAD → " } else { "" };
    let local_marker = if commit.is_local { "●" } else { "○" };
    let branch_tag = if !commit.branch_tag.is_empty() {
        format!(" ({})", commit.branch_tag)
    } else {
        String::new()
    };
    
    println!(
        "{} {} {}{}{} - {}",
        local_marker,
        commit.short_hash,
        head_marker,
        commit.message,
        branch_tag,
        commit.author
    );
}

// Output:
// ● a1b2c3d HEAD → feat: Add documentation (main) - Luka Gray
// ○ d4e5f6a fix: Resolve merge conflict (develop) - Luka Gray
// ○ 7g8h9i0 refactor: Clean up parser - Luka Gray
```

### Multi-Repository Workspace

```rust
let mut registry = RepoRegistry::new();

// Discover repos
let repos = registry.discover_workspace_repos("D:/Projects");
println!("Found {} repositories", repos.len());

for repo in &repos {
    let active = if repo.is_active { "[ACTIVE]" } else { "" };
    let changes = if repo.has_changes { "●" } else { "○" };
    println!("{} {} {} - {}", 
        changes, 
        active, 
        repo.name, 
        repo.root.display()
    );
}

// Switch active repo
registry.set_active("D:/Projects/operon-rs")?;

// Get active repo
if let Some(active) = registry.active_repo() {
    let workspace = active.root.clone();
    let stats = get_diff_stats_async(workspace).await?;
    println!("Active repo changes: +{} -{}", 
        stats.insertions, 
        stats.deletions
    );
}
```

---

## Performance Characteristics

| Operation | Complexity | Typical Time |
|-----------|-----------|--------------|
| **Repository discovery** | O(1) disk seek | <1ms |
| **Diff stats** | O(changed lines) | 5-50ms |
| **Diff details** (100 files) | O(files × hunks × lines) | 50-200ms |
| **Stage file** | O(file size) | 1-10ms |
| **Unstage file** | O(file size) | 1-10ms |
| **Revert file** | O(file size) | 1-10ms |
| **Stage hunk** | O(file hunks) + patch apply | 5-20ms |
| **Commit** | O(index size) | 10-100ms |
| **List branches** | O(branches × upstream queries) | 10-50ms |
| **Switch branch** | O(changed files) | 50-500ms |
| **Commit graph** (50 commits) | O(commits × branch lookups) | 20-100ms |
| **Push** | O(unpushed commits × network) | 500ms-5s |
| **Fetch** | O(refs × network) | 200ms-2s |
| **Pull** | fetch + O(merge analysis) | 500ms-3s |

**Optimization Notes**:
- `get_diff_stats` is **much faster** than `get_diff_details` (no hunk parsing)
- Commit graph with `limit` avoids full repository traversal
- Async wrappers add ~1-2ms overhead (thread pool dispatch)

---

## Testing

```bash
# Run all tests
cargo test -p operon-diff

# Test specific modules
cargo test -p operon-diff --test status
cargo test -p operon-diff --test stage
cargo test -p operon-diff --test branch
cargo test -p operon-diff --test graph

# Test with output
cargo test -p operon-diff -- --nocapture
```

---

## Dependencies

```toml
[dependencies]
git2 = "0.19"              # libgit2 bindings
auth-git2 = "0.5"          # SSH/HTTPS credential management
tokio = { version = "1", features = ["rt", "macros"] }
serde = { version = "1", features = ["derive"] }
thiserror = "1"            # Error derive macros
```

**Why libgit2?**
- Native performance (no shell process spawning)
- Fine-grained control (patch application, revwalk, credentials)
- Portable (works without Git CLI installed)
- Thread-safe (multiple repos simultaneously)

---

## License

Operon is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

---

> Built by **Luka Gray (aka Soumo Mukherjee)** • West Bengal, India • 2026
