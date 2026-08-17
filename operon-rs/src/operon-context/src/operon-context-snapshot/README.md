# operon-context-snapshot

**Per-turn system message builder with intelligent caching and filesystem watch**

`operon-context-snapshot` generates the LLM system message for each conversation turn, combining agent identity, workspace rules, project structure, git status, and available tools into a single plain-text context block.

---

## Overview

Every LLM request needs a **fresh system message** reflecting current environment state. This crate builds that message by assembling five blocks:

```mermaid
flowchart LR
    A[Bootstrap<br/>Always Fresh] --> Render
    B[AGENTS.md<br/>Cached] --> Render
    C[Channel Context<br/>Static] --> Render
    D[Directory Tree<br/>Cached] --> Render
    E[Git Status<br/>Always Fresh] --> Render
    F[Tool Groups<br/>Static] --> Render
    Render --> G[System Message<br/>Plain Text]
    
    style A fill:#FFD700
    style E fill:#FFD700
    style B fill:#87CEEB
    style D fill:#87CEEB
    style C fill:#90EE90
    style F fill:#90EE90
    style G fill:#90EE90
```

**Key Features**:
- **Filesystem watcher**: Invalidates caches when `AGENTS.md` or workspace root changes
- **Selective caching**: Tree and AGENTS.md cached; bootstrap and git always fresh
- **Zero I/O blocks**: Channel context and tool groups are in-memory strings
- **Single entry point**: `SnapshotBuilder::build() -> SessionSnapshot`

---

## Architecture

### SnapshotBuilder Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Created: new(config)
    Created --> Watching: Start filesystem watcher
    Watching --> BuildRequested: build() called
    BuildRequested --> CheckCache: Check dirty flags
    CheckCache --> ReadTree: tree_dirty = true
    CheckCache --> ReadAgents: agents_md_dirty = true
    CheckCache --> ReuseCache: All clean
    ReadTree --> ComputeGit
    ReadAgents --> ComputeGit
    ReuseCache --> ComputeGit
    ComputeGit --> Assemble: Combine blocks
    Assemble --> Render: SessionSnapshot
    Render --> Watching: Return to caller
    Watching --> Invalidated: File change event
    Invalidated --> Watching: Set dirty flags
    
    style Created fill:#87CEEB
    style Watching fill:#90EE90
    style Invalidated fill:#FFD700
```

### Block Computation Strategy

| Block | Computation | Caching | Invalidation |
|-------|-------------|---------|--------------|
| **Bootstrap** | Always fresh | None | Every `build()` |
| **AGENTS.md** | Read from `$ROOT/AGENTS.md` | Cached | `AGENTS.md` modified |
| **Channel Context** | In-memory string (config) | None | Manual via `config_mut()` |
| **Tree** | Gitignore-aware traversal | Cached | Workspace root change |
| **Git Status** | libgit2 query | None | Every `build()` |
| **Tool Groups** | In-memory vec (config) | None | Manual via `config_mut()` |

---

## Snapshot Blocks

### Block 1: Bootstrap (Always Fresh)

**Purpose**: Session identity and timestamp

```rust
pub struct BootstrapBlock {
    pub agent_name: String,      // "Operon"
    pub timestamp: String,        // RFC3339 UTC: "2026-08-17T14:32:19Z"
    pub session_id: String,       // Hex nanoseconds
    pub role: Role,               // Owner | External
    pub system_prompt: &'static str, // Main agent instructions
}
```

**Rendered Output**:

```
=== OPERON SESSION ===
Agent: Operon
Role: Owner
Session: 18f4a2b3c9d1e
Time: 2026-08-17T14:32:19Z
```

**Timestamp Generation**: Custom UTC formatter using civil calendar algorithm (no chrono dependency)

---

### Block 2: AGENTS.md (Cached)

**Purpose**: Workspace-specific rules and instructions

```mermaid
flowchart TD
    A[build called] --> B{agents_md_dirty?}
    B -->|true| C[Read AGENTS.md from disk]
    B -->|false| D[Use cached value]
    C --> E{File exists?}
    E -->|Yes| F[Cache Some contents]
    E -->|No| G[Cache None]
    F --> H[Return cached]
    G --> H
    D --> H
    
    style C fill:#FFD700
    style F fill:#87CEEB
```

**Invalidation Trigger**: Filesystem watcher detects change to any file named `AGENTS.md` in workspace

**Rendered Output**:

```
=== INSTRUCTIONS ===
You are an AI agent operating as part of a production-grade software system...
[full AGENTS.md content]
```

**Fallback**: If missing: `(none)`

---

### Block 3: Channel Context (Static)

**Purpose**: Channel-specific role instructions (WhatsApp, Telegram, etc.)

```rust
pub struct SnapshotConfig {
    pub channel_instructions: Option<String>, // In-memory string
}
```

**Rendered Output** (when present):

```
=== CHANNEL CONTEXT ===
WhatsApp channel constraints:
- Owner: Full tool access
- External: No filesystem tools
```

**Update Strategy**: Manual via `builder.config_mut().channel_instructions = Some(...)`

---

### Block 4: Directory Tree (Cached)

**Purpose**: Gitignore-aware workspace structure

```mermaid
flowchart TD
    A[build called] --> B{tree_dirty?}
    B -->|true| C[Scan workspace root]
    B -->|false| D[Use cached DirectoryTree]
    C --> E[Read .gitignore rules]
    E --> F[Traverse up to depth levels]
    F --> G[Sort: dirs first, then files]
    G --> H[Render hierarchical tree]
    H --> I[Cache DirectoryTree]
    I --> J[Return cached]
    D --> J
    
    style C fill:#FFD700
    style I fill:#87CEEB
```

**Configuration**:

```rust
pub struct SnapshotConfig {
    pub tree_depth: usize, // Default: 1 (root + immediate children)
}
```

**Invalidation Trigger**: Filesystem watcher detects file/directory create/modify/remove in workspace root

**Rendered Output** (depth=1):

```
=== PROJECT ===
Root: D:\Operon
d:\Operon
├── .git\
├── .github\
├── assets\
├── operon-rs\
├── .gitignore
├── AGENTS.md
├── Cargo.toml
└── README.md
```

**Sorting**: Directories first (alphabetical), then files (alphabetical)

**Gitignore Integration**: Uses `ignore` crate (same as ripgrep) to respect `.gitignore`, `.git/info/exclude`, and global ignore

---

### Block 5: Git Status (Always Fresh)

**Purpose**: Current repository state

```rust
pub struct GitStatus {
    pub branch: String,      // "main", "HEAD (detached)"
    pub staged: usize,       // Files in index
    pub unstaged: usize,     // Modified working tree
    pub untracked: usize,    // New files not in .gitignore
    pub insertions: u64,     // Lines added
    pub deletions: u64,      // Lines removed
}
```

**Computation**: libgit2 queries repository state

**Rendered Output**:

```
=== GIT ===
Branch: feat/documentation
Staged: 3  Unstaged: 1  Untracked: 0
Modified lines: +142 -37
```

**Omitted**: If workspace root is not a git repository

---

### Block 6: Tool Groups (Static)

**Purpose**: List available tool categories

```rust
pub struct SnapshotConfig {
    pub tool_groups: Vec<String>, // ["fs", "shell", "web", "todo"]
}
```

**Rendered Output**:

```
=== AVAILABLE TOOLS ===
fs, shell, web, todo, memory, media
```

**Omitted**: If `tool_groups` is empty

**Update Strategy**: Manual via `builder.config_mut().tool_groups = vec![...]`

---

## Usage

### Basic Setup

```rust
use operon_context_snapshot::{SnapshotBuilder, SnapshotConfig, Role};
use std::path::PathBuf;

// Configure builder
let config = SnapshotConfig {
    root: PathBuf::from("D:/Operon"),
    role: Role::Owner,
    session_id: "session_abc123".to_string(),
    tree_depth: 2,
    tool_groups: vec![
        "fs".to_string(),
        "shell".to_string(),
        "web".to_string(),
    ],
    channel_instructions: None,
};

// Create builder (starts filesystem watcher)
let mut builder = SnapshotBuilder::new(config)?;

// Build snapshot for current turn
let snapshot = builder.build()?;

// Use in system message
let system_message = snapshot.render();
println!("{}", system_message);
```

### Per-Turn Workflow

```mermaid
sequenceDiagram
    participant Runner as SessionRunner
    participant Builder as SnapshotBuilder
    participant Watcher as Filesystem Watcher
    participant Disk
    
    Runner->>Builder: new(config)
    Builder->>Watcher: Start watching root
    Watcher-->>Builder: Ready
    
    Runner->>Builder: build()
    Builder->>Builder: Assemble bootstrap (fresh)
    Builder->>Builder: Check agents_md_dirty
    alt Cache clean
        Builder->>Builder: Reuse cached AGENTS.md
    else Cache dirty
        Builder->>Disk: Read AGENTS.md
        Disk-->>Builder: Content
        Builder->>Builder: Update cache
    end
    
    Builder->>Builder: Check tree_dirty
    alt Cache clean
        Builder->>Builder: Reuse cached tree
    else Cache dirty
        Builder->>Disk: Scan workspace
        Disk-->>Builder: Tree entries
        Builder->>Builder: Update cache
    end
    
    Builder->>Disk: Query git status (libgit2)
    Disk-->>Builder: GitStatus
    
    Builder->>Builder: Assemble SessionSnapshot
    Builder-->>Runner: snapshot
    
    Runner->>Runner: sanitize + send to LLM
    
    Note over Watcher,Disk: File change occurs
    Disk->>Watcher: AGENTS.md modified
    Watcher->>Builder: Set agents_md_dirty = true
    
    Runner->>Builder: build() (next turn)
    Builder->>Disk: Re-read AGENTS.md (dirty flag set)
```

### Dynamic Configuration Updates

```rust
// Update role mid-session (channel role change)
builder.config_mut().role = Role::External;

// Add channel-specific instructions
builder.config_mut().channel_instructions = Some(
    "External role: Filesystem tools disabled".to_string()
);

// Update available tool groups
builder.config_mut().tool_groups.push("memory".to_string());

// Increase tree depth for deep exploration
builder.config_mut().tree_depth = 3;

// Next build() will use updated config
let snapshot = builder.build()?;
```

---

## Caching Strategy

### Cache Invalidation Flow

```mermaid
flowchart TD
    A[File Event] --> B{Event type}
    B -->|Create/Modify/Remove| C{Affects workspace root?}
    B -->|Access/Open| D[Ignore]
    C -->|Yes| E[Set tree_dirty = true]
    C -->|No| F{Filename == AGENTS.md?}
    F -->|Yes| G[Set agents_md_dirty = true]
    F -->|No| D
    E --> H[Next build will refresh]
    G --> H
    
    style E fill:#FFD700
    style G fill:#FFD700
    style D fill:#90EE90
```

### Cache Hit Rates (Typical Session)

| Turn | Bootstrap | AGENTS.md | Tree | Git | Result |
|------|-----------|-----------|------|-----|--------|
| 1 | Fresh | Read | Read | Fresh | Full computation |
| 2-10 | Fresh | **Cache hit** | **Cache hit** | Fresh | Fast path |
| 11 | Fresh | **Cache hit** (edit detected) | Read | Fresh | Tree refresh |
| 12-20 | Fresh | **Cache hit** | **Cache hit** | Fresh | Fast path |

**Expected**: 80-90% cache hit rate for AGENTS.md and tree in typical sessions

---

## Filesystem Watcher Details

### Watcher Backend

- **Windows**: ReadDirectoryChangesW (native API)
- **Linux**: inotify
- **macOS**: FSEvents

**Crate**: `notify` (recommended_watcher with automatic backend selection)

### Watch Scope

```rust
watcher.watch(&config.root, RecursiveMode::NonRecursive)
```

**NonRecursive**: Only monitors workspace root (not subdirectories)

**Rationale**: Subdirectory changes don't affect tree (only root-level structure matters at depth=1-2)

### Event Filtering

```rust
fn event_should_invalidate(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
    )
}
```

**Ignored Events**: Access, Metadata (no content change)

### Error Handling

```mermaid
flowchart TD
    A[Watcher Error] --> B{Error Type}
    B -->|Backend failure| C[Set all dirty flags]
    B -->|Event parsing error| C
    C --> D[Next build refreshes everything]
    D --> E[Log warning]
    
    style C fill:#FFD700
```

**Conservative Fallback**: On watcher errors, mark all caches dirty

---

## Thread Safety

```rust
impl SnapshotBuilder {
    // ✅ Send: Can be moved across threads
    // ❌ NOT Sync: Cannot be shared across threads
}
```

**Multi-threaded Usage**:

```rust
use std::sync::Mutex;
use tokio::sync::Mutex as TokioMutex;

// Async context
let builder = Arc::new(TokioMutex::new(SnapshotBuilder::new(config)?));

// Tokio task
let snapshot = builder.lock().await.build()?;

// Std threads
let builder = Arc::new(Mutex::new(SnapshotBuilder::new(config)?));
let snapshot = builder.lock().unwrap().build()?;
```

**Why Not Sync**: Internal `RecommendedWatcher` is not `Sync` (platform-specific file handles)

---

## Performance

| Operation | Complexity | Typical Time |
|-----------|-----------|--------------|
| **Builder creation** | O(1) + watcher setup | 1-2ms |
| **build() - all cache hits** | O(1) | <1ms |
| **build() - AGENTS.md miss** | O(file size) | 1-5ms |
| **build() - tree miss (depth=1)** | O(root entries × gitignore rules) | 5-20ms |
| **build() - tree miss (depth=2)** | O(root entries × avg_children × rules) | 20-100ms |
| **build() - git status** | O(repo size) | 5-50ms |

**Typical Turn (cache hits)**: 10-60ms total (dominated by git status query)

**First Turn (all misses)**: 50-200ms

---

## Error Handling

```mermaid
flowchart TD
    A[Operation] --> B{Error Type}
    B -->|InvalidRoot| C[Root path doesn't exist]
    B -->|Io| D[Disk read failed]
    B -->|GitError| E[libgit2 query failed]
    B -->|WatchError| F[Watcher setup failed]
    
    C --> G[Return SnapshotError]
    D --> G
    E --> G
    F --> G
    
    G --> H{Caller Handling}
    H -->|SessionRunner| I[Emit PreTurnFailed<br/>Transition to Failed state]
    H -->|Test| J[Propagate error]
    
    style G fill:#FF6B6B
    style I fill:#FFD700
```

### Error Types

| Error | Cause | Recovery |
|-------|-------|----------|
| `InvalidRoot` | Config root path doesn't exist | Fix config, retry |
| `Io` | Disk read permission denied, file deleted mid-read | Retry, fallback to partial snapshot |
| `GitError` | Corrupted git repo, permission issues | Omit git block, continue |
| `WatchError` | Too many open files, unsupported filesystem | Disable caching, always recompute |

---

## Integration with SessionRunner

```mermaid
sequenceDiagram
    participant Runner as SessionRunner
    participant Builder as SnapshotBuilder
    participant Sanitizer
    participant Provider as NormalizeProvider
    participant LLM
    
    Runner->>Runner: Session starts
    Runner->>Builder: new(config)
    Builder-->>Runner: builder (watcher active)
    
    loop Each Turn
        Runner->>Runner: User sends message
        Runner->>Builder: build()
        Builder-->>Runner: SessionSnapshot
        
        Runner->>Sanitizer: sanitize(messages, snapshot, role)
        Sanitizer-->>Runner: Clean messages
        
        Runner->>Provider: convert_to_provider(messages)
        Provider-->>Runner: Provider-specific format
        
        Runner->>LLM: Send request with fresh system message
        LLM-->>Runner: Response
    end
```

**SessionRunner Responsibilities**:
1. Create `SnapshotBuilder` at session start
2. Call `builder.build()` before each LLM request
3. Pass snapshot to `sanitizer::sanitize()`
4. Update `config_mut()` when role or tools change

---

## Example Rendered Snapshot

```
You are Operon, an AI-powered development environment...
[full system prompt]

=== OPERON SESSION ===
Agent: Operon
Role: Owner
Session: 18f4a2b3c9d1e
Time: 2026-08-17T14:32:19Z

=== INSTRUCTIONS ===
You are an AI agent operating as part of a production-grade software system.
Your behavior must reflect real-world engineering standards...
[full AGENTS.md content]

=== CHANNEL CONTEXT ===
WhatsApp channel: Owner role has full tool access.
External contacts are restricted to read-only operations.

=== PROJECT ===
Root: D:\Operon
d:\Operon
├── .git\
├── .github\
│   └── workflows\
├── assets\
│   ├── lucide\
│   └── logo.svg
├── operon-rs\
│   ├── src\
│   └── Cargo.toml
├── .gitignore
├── AGENTS.md
├── Cargo.toml
└── README.md

=== GIT ===
Branch: feat/documentation
Staged: 3  Unstaged: 1  Untracked: 0
Modified lines: +142 -37

=== AVAILABLE TOOLS ===
fs, shell, web, todo, memory, media
```

---

## Testing

```bash
# Run all tests
cargo test -p operon-context-snapshot

# Test specific blocks
cargo test -p operon-context-snapshot --test tree
cargo test -p operon-context-snapshot --test git
cargo test -p operon-context-snapshot --test bootstrap
```

**Test Coverage**:
- ✅ Bootstrap timestamp format (RFC3339 compliance)
- ✅ Tree rendering with gitignore
- ✅ Git status computation (staged, unstaged, untracked, line counts)
- ✅ Snapshot render output format
- ✅ Cache invalidation logic
- ✅ Multi-turn builder reuse

---

## License

Operon is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

---

> Built by **Luka Gray (aka Soumo Mukherjee)** • West Bengal, India • 2026
