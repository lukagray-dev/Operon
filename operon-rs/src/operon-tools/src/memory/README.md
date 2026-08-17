# operon-tools-memory

**Global persistent memory system — SQLite-backed agent memory that survives restarts**

`operon-tools-memory` provides a complete memory subsystem for the Operon agent: **5 tools** (add, edit, delete, retrieve, search) backed by a **global SQLite store** with **FTS5 full-text search**.

Unlike session-scoped todos, memories persist **indefinitely across all sessions** until explicitly deleted.

---

## Overview

```mermaid
flowchart TB
    Model[Model calls memory tool] --> Facade[operon-tools-memory]
    Facade --> Add[memory_add]
    Facade --> Edit[memory_edit]
    Facade --> Delete[memory_delete]
    Facade --> Retrieve[memory_retrieve]
    Facade --> Search[memory_search]
    
    Add --> Store[MemoryStore<br/>SQLite + FTS5]
    Edit --> Store
    Delete --> Store
    Retrieve --> Store
    Search --> Store
    
    Store --> DB[(~/.operon/memory/<br/>memory.db)]
    
    style Facade fill:#FFD700
    style Store fill:#90EE90
    style DB fill:#87CEEB
```

---

## Key Features

- ✅ **5 CRUD + Search tools** — add, edit, delete, retrieve, search
- ✅ **Global persistence** — survives restarts, not session-scoped
- ✅ **FTS5 full-text search** — BM25 relevance ranking
- ✅ **Auto-sync triggers** — FTS index stays in sync with inserts/updates/deletes
- ✅ **Tag support** — categorize memories with string tags
- ✅ **SQLite WAL mode** — concurrent read/write without blocking
- ✅ **Clone-safe store** — `MemoryStore` wraps `sqlx::SqlitePool` (internally Arc)

---

## Architecture

### Crate Dependency Graph

```mermaid
flowchart TB
    Facade[operon-tools-memory<br/>Facade] --> Add[memory_add]
    Facade --> Edit[memory_edit]
    Facade --> Delete[memory_delete]
    Facade --> Retrieve[memory_retrieve]
    Facade --> Search[memory_search]
    
    Add --> Store[memory_store<br/>Leaf crate]
    Edit --> Store
    Delete --> Store
    Retrieve --> Store
    Search --> Store
    
    Store --> SQLx[sqlx + sqlite]
    Store --> Config[operon-config]
    
    Add --> Core[operon-tools-core]
    Edit --> Core
    Delete --> Core
    Retrieve --> Core
    Search --> Core
    
    style Facade fill:#FFD700
    style Store fill:#90EE90
```

**Leaf Crate**: `operon-tools-memory-store` is the **only** crate that depends on `sqlx`. All 5 tool crates depend on the store, creating a clean dependency tree with no cycles.

---

### Database Schema

```mermaid
erDiagram
    memories {
        INTEGER id PK
        TEXT content
        TEXT tags
        TEXT created_at
        TEXT updated_at
    }
    
    memories_fts {
        INTEGER rowid FK
        TEXT content
    }
    
    memories ||--o{ memories_fts : "FTS5 index"
```

**DDL**:
```sql
CREATE TABLE memories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    content    TEXT    NOT NULL,
    tags       TEXT    NOT NULL DEFAULT '[]',  -- JSON array
    created_at TEXT    NOT NULL,               -- RFC3339
    updated_at TEXT    NOT NULL                -- RFC3339
);

CREATE VIRTUAL TABLE memories_fts USING fts5(
    content,
    content='memories',
    content_rowid='id'
);
```

**Triggers** (auto-sync FTS):
```sql
-- Insert trigger
CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, content) VALUES (new.id, new.content);
END;

-- Delete trigger
CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content) 
    VALUES ('delete', old.id, old.content);
END;

-- Update trigger
CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content) 
    VALUES ('delete', old.id, old.content);
    INSERT INTO memories_fts(rowid, content) VALUES (new.id, new.content);
END;
```

---

## Core Types

### Memory

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,              // "1", "2", etc. (i64 → String)
    pub content: String,          // The fact/preference/note
    pub tags: Vec<String>,        // ["preference", "workflow"]
    pub created_at: String,       // RFC3339: "2024-01-15T10:30:00+00:00"
    pub updated_at: String,       // RFC3339: "2024-01-15T10:30:00+00:00"
}
```

---

### MemoryStore

```rust
#[derive(Clone)]
pub struct MemoryStore {
    pool: SqlitePool,  // Internally Arc — Clone is O(1)
}

impl MemoryStore {
    pub async fn connect(db_path: &Path) -> Result<Self, MemoryStoreError>;
    pub async fn connect_default() -> Result<Self, MemoryStoreError>;
    
    // Write operations
    pub async fn add(&self, content: String, tags: Vec<String>) 
        -> Result<Memory, MemoryStoreError>;
    pub async fn edit(&self, id: &str, content: Option<String>, tags: Option<Vec<String>>) 
        -> Result<Option<Memory>, MemoryStoreError>;
    pub async fn delete(&self, id: &str) 
        -> Result<bool, MemoryStoreError>;
    
    // Read operations
    pub async fn get(&self, id: &str) 
        -> Result<Option<Memory>, MemoryStoreError>;
    pub async fn list(&self, limit: usize, offset: usize) 
        -> Result<Vec<Memory>, MemoryStoreError>;
    pub async fn count(&self) 
        -> Result<i64, MemoryStoreError>;
    pub async fn search(&self, query: &str, limit: usize) 
        -> Result<Vec<Memory>, MemoryStoreError>;
}
```

---

### MemoryStoreError

```rust
#[derive(Debug, Error)]
pub enum MemoryStoreError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("failed to create memory directory: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("failed to resolve default memory path: {0}")]
    Config(#[from] operon_config::ConfigError),
}
```

---

## Tool Catalog

### 1. memory_add

**Purpose**: Store a new persistent memory

```mermaid
flowchart TB
    Start[memory_add called] --> Parse[Parse MemoryAddArgs]
    Parse --> Validate{content empty?}
    Validate -->|Yes| Err[Return error]
    Validate -->|No| Insert[MemoryStore::add]
    Insert --> Trigger[INSERT trigger fires]
    Trigger --> FTS[FTS index updated]
    FTS --> Return[Return Memory]
    
    style Insert fill:#90EE90
```

**Input**:
```json
{
  "content": "User prefers dark mode",
  "tags": ["preference"]
}
```

**Output**:
```json
{
  "memory": {
    "id": "1",
    "content": "User prefers dark mode",
    "tags": ["preference"],
    "created_at": "2024-01-15T10:30:00+00:00",
    "updated_at": "2024-01-15T10:30:00+00:00"
  },
  "total": 1
}
```

**Aliases**: `content` = `note`, `fact`, `text`, `memory`, `info`; `tags` = `tag`

---

### 2. memory_edit

**Purpose**: Partially update an existing memory

```mermaid
flowchart TB
    Start[memory_edit called] --> Parse[Parse MemoryEditArgs]
    Parse --> Fetch[Fetch existing memory]
    Fetch --> Check{Memory exists?}
    Check -->|No| NotFound[Return error]
    Check -->|Yes| Apply[Apply partial updates]
    Apply --> Update[UPDATE query]
    Update --> Trigger[UPDATE trigger fires]
    Trigger --> FTS[FTS re-indexed]
    FTS --> Refetch[Re-fetch updated memory]
    Refetch --> Return[Return Memory]
    
    style Apply fill:#FFD700
    style Update fill:#90EE90
```

**Input** (partial update):
```json
{
  "id": "1",
  "content": "User strongly prefers dark mode"
}
```

**Output**:
```json
{
  "memory": {
    "id": "1",
    "content": "User strongly prefers dark mode",
    "tags": ["preference"],  // Unchanged
    "created_at": "2024-01-15T10:30:00+00:00",  // Unchanged
    "updated_at": "2024-01-15T11:00:00+00:00"   // Updated!
  }
}
```

**Validation**:
- At least one of `content` or `tags` must be provided
- `content` cannot be empty string

---

### 3. memory_delete

**Purpose**: Permanently remove a memory

```mermaid
flowchart TB
    Start[memory_delete called] --> Parse[Parse MemoryDeleteArgs]
    Parse --> Delete[DELETE FROM memories WHERE id = ?]
    Delete --> Trigger[DELETE trigger fires]
    Trigger --> FTS[FTS entry removed]
    FTS --> Check{rows_affected > 0?}
    Check -->|Yes| Count[Get remaining count]
    Check -->|No| NotFound[Return error]
    Count --> Return[Return id + remaining]
    
    style Delete fill:#FF6B6B
```

**Input**:
```json
{
  "id": "1"
}
```

**Output**:
```json
{
  "id": "1",
  "remaining": 4
}
```

**Note**: This is **irreversible** — the memory is permanently deleted from SQLite.

---

### 4. memory_retrieve

**Purpose**: Fetch one memory by ID, or list all with pagination

```mermaid
stateDiagram-v2
    [*] --> CheckMode: memory_retrieve
    
    CheckMode --> SingleMode: id present
    CheckMode --> ListMode: id absent
    
    SingleMode --> FetchOne: SELECT WHERE id = ?
    FetchOne --> Found: Memory exists
    FetchOne --> NotFound: Memory not found
    
    ListMode --> FetchAll: SELECT ... ORDER BY created_at DESC<br/>LIMIT ? OFFSET ?
    FetchAll --> ReturnList: Vec<Memory>
    
    Found --> [*]
    NotFound --> [*]
    ReturnList --> [*]
```

**Single-record mode**:
```json
{
  "id": "1"
}
```

**Output**:
```json
{
  "memories": [{
    "id": "1",
    "content": "User prefers dark mode",
    "tags": ["preference"],
    "created_at": "...",
    "updated_at": "..."
  }],
  "total": 12,
  "limit": 1,
  "offset": 0
}
```

**List mode**:
```json
{
  "limit": 10,
  "offset": 0
}
```

**Output**:
```json
{
  "memories": [...],  // 10 most recent
  "total": 42,
  "limit": 10,
  "offset": 0
}
```

---

### 5. memory_search

**Purpose**: Full-text search with FTS5 BM25 ranking

```mermaid
flowchart TB
    Start[memory_search called] --> Parse[Parse MemorySearchArgs]
    Parse --> Check{query empty?}
    Check -->|Yes| Empty[Return empty vec]
    Check -->|No| FTS[FTS5 MATCH query]
    FTS --> Rank[ORDER BY rank]
    Rank --> Limit[LIMIT ?]
    Limit --> Join[JOIN memories table]
    Join --> Return[Return Vec<Memory>]
    
    style FTS fill:#FFD700
    style Rank fill:#90EE90
```

**Input**:
```json
{
  "query": "dark mode",
  "limit": 10
}
```

**Output**:
```json
{
  "memories": [
    {
      "id": "1",
      "content": "User prefers dark mode",
      "tags": ["preference"],
      "created_at": "...",
      "updated_at": "..."
    },
    {
      "id": "7",
      "content": "Always use dark mode in IDE",
      "tags": ["workflow"],
      "created_at": "...",
      "updated_at": "..."
    }
  ],
  "count": 2,
  "query": "dark mode"
}
```

**FTS5 Syntax Support**:
```json
{"query": "Rust"}                   // Single term
{"query": "Rust AND programming"}   // AND (implicit: "Rust programming")
{"query": "Rust OR Go"}             // OR
{"query": "\"dark mode\""}          // Exact phrase
```

**Relevance Ranking**: BM25 algorithm, ascending `rank` = most relevant first

---

## Store Initialization

### Default Path

```mermaid
flowchart TB
    Start[MemoryStore::connect_default] --> Resolve[OperonPaths::resolve]
    Resolve --> Path[~/.operon/memory/memory.db]
    Path --> Ensure[ensure_dirs_exist]
    Ensure --> Connect[connect with path]
    Connect --> Schema[Run DDL statements]
    Schema --> WAL[Enable WAL mode]
    WAL --> Pool[Return SqlitePool]
    
    style Path fill:#FFD700
    style Pool fill:#90EE90
```

**Platform Paths**:
| Platform | Path |
|----------|------|
| **Linux** | `~/.operon/memory/memory.db` |
| **macOS** | `~/.operon/memory/memory.db` |
| **Windows** | `C:\Users\<user>\.operon\memory\memory.db` |

---

### Custom Path (Testing)

```rust
use operon_tools_memory_store::MemoryStore;
use tempfile::tempdir;

#[tokio::test]
async fn test_memory_add() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test.db");
    
    let store = MemoryStore::connect(&db_path).await.unwrap();
    
    let memory = store.add(
        "Test memory".to_string(),
        vec!["test".to_string()]
    ).await.unwrap();
    
    assert_eq!(memory.id, "1");
}
```

---

## SQLite Configuration

### Connection Options

```rust
SqliteConnectOptions::new()
    .filename(db_path)              // Uses Path directly (no URI issues on Windows)
    .create_if_missing(true)        // Auto-create if first run
    .journal_mode(SqliteJournalMode::Wal)  // Write-Ahead Log for concurrency
```

### WAL Mode Benefits

```mermaid
graph LR
    A[WAL Mode] --> B[Concurrent reads]
    A --> C[Non-blocking writes]
    A --> D[Better crash recovery]
    
    E[Rollback Mode] --> F[Exclusive write lock]
    E --> G[Blocks all reads]
    
    style A fill:#90EE90
    style E fill:#FF6B6B
```

---

## FTS5 Implementation

### Trigger Synchronization

```mermaid
sequenceDiagram
    participant App as Application
    participant DB as memories table
    participant Trigger as SQLite Trigger
    participant FTS as memories_fts
    
    App->>DB: INSERT new memory
    DB->>Trigger: AFTER INSERT fires
    Trigger->>FTS: INSERT INTO memories_fts
    FTS-->>App: Row visible in search
    
    App->>DB: UPDATE memory content
    DB->>Trigger: AFTER UPDATE fires
    Trigger->>FTS: DELETE old + INSERT new
    FTS-->>App: Updated row in search
    
    App->>DB: DELETE memory
    DB->>Trigger: AFTER DELETE fires
    Trigger->>FTS: DELETE FROM memories_fts
    FTS-->>App: Row removed from search
```

**Guarantee**: FTS index is **always in sync** — no manual rebuild needed.

---

### Search Query Flow

```sql
-- User query: "dark mode"
SELECT m.id, m.content, m.tags, m.created_at, m.updated_at
FROM memories m
INNER JOIN memories_fts ON memories_fts.rowid = m.id
WHERE memories_fts MATCH 'dark mode'
ORDER BY rank
LIMIT 10;
```

**FTS5 Magic**:
- `MATCH` clause uses BM25 tokenization
- `rank` pseudo-column provides relevance score
- Ascending `rank` = most relevant first (lower rank = better match)

---

## Tag Storage (JSON Text)

### Why JSON Text?

```mermaid
flowchart LR
    A[❌ Separate tags table] --> B[JOIN overhead]
    A --> C[Complex queries]
    A --> D[More transactions]
    
    E[✅ JSON TEXT column] --> F[Single row read]
    E --> G[Simple queries]
    E --> H[Deserialize in Rust]
    
    style A fill:#FF6B6B
    style E fill:#90EE90
```

**Implementation**:
```rust
// Serialize on insert
let tags_json = serde_json::to_string(&tags)?;  // '["pref","workflow"]'
sqlx::query("INSERT INTO memories (tags, ...) VALUES (?, ...)")
    .bind(&tags_json)
    .execute(&pool).await?;

// Deserialize on read
let tags: Vec<String> = serde_json::from_str(&row.tags)?;
```

---

## Concurrency Model

### Clone-Safe Store

```rust
#[derive(Clone)]
pub struct MemoryStore {
    pool: SqlitePool,  // Arc<PoolInner> internally
}
```

**Clone Cost**: O(1) — just increments Arc refcount

**Thread Safety**:
```rust
// All methods take &self, not &mut self
pub async fn add(&self, ...) -> Result<Memory, ...>
pub async fn search(&self, ...) -> Result<Vec<Memory>, ...>
```

**Pool Handles Concurrency**:
- Max 4 connections by default
- Automatic connection reuse
- Transparent locking (no `Mutex` in app code)

---

### Concurrent Access Example

```rust
use operon_tools_memory_store::MemoryStore;

#[tokio::main]
async fn main() {
    let store = MemoryStore::connect_default().await.unwrap();
    
    // Clone is cheap — just an Arc refcount bump
    let store1 = store.clone();
    let store2 = store.clone();
    
    // Concurrent reads/writes across tasks
    let task1 = tokio::spawn(async move {
        store1.add("Memory 1".to_string(), vec![]).await
    });
    
    let task2 = tokio::spawn(async move {
        store2.search("query", 10).await
    });
    
    let (mem1, results) = tokio::join!(task1, task2);
}
```

---

## Error Handling

### Store-Level Errors

```mermaid
flowchart TB
    Op[Store operation] --> Res{Result?}
    
    Res -->|Ok| Success[Return data]
    
    Res -->|Err| Type{Error type?}
    Type -->|Database| SQL[SQLite error<br/>constraint violation, etc.]
    Type -->|Io| Dir[Directory creation failed]
    Type -->|Config| Home[HOME env var missing]
    
    SQL --> Prop[Propagate to tool]
    Dir --> Prop
    Home --> Prop
    
    Prop --> Tool[Tool returns error ToolResult]
    
    style Success fill:#90EE90
    style SQL fill:#FF6B6B
    style Dir fill:#FF6B6B
    style Home fill:#FF6B6B
```

---

### Tool-Level Error Handling

Each tool wraps store errors into domain-specific error types:

```rust
// memory_add
pub enum MemoryAddToolError {
    ArgsParse(#[from] serde_json::Error),
    // Store errors become in-band ToolResult errors
}

// memory_edit
pub enum MemoryEditToolError {
    ArgsParse(#[from] serde_json::Error),
    // Store errors become in-band ToolResult errors
}
```

**Pattern**: Store errors are converted to `ToolResult::error` (not propagated as `Err`)

---

## Usage Examples

### Adding Memories

```rust
use operon_tools_memory_store::MemoryStore;

let store = MemoryStore::connect_default().await?;

// Add a preference
let pref = store.add(
    "User prefers dark mode".to_string(),
    vec!["preference".to_string()]
).await?;

// Add a fact with multiple tags
let fact = store.add(
    "This project uses AGPL-3.0".to_string(),
    vec!["project".to_string(), "license".to_string()]
).await?;
```

---

### Searching Memories

```rust
// Find all memories mentioning "dark mode"
let results = store.search("dark mode", 10).await?;

for memory in results {
    println!("[{}] {}", memory.id, memory.content);
    println!("  Tags: {:?}", memory.tags);
}
```

---

### Partial Updates

```rust
// Update only content
store.edit("1", Some("Updated content".to_string()), None).await?;

// Update only tags
store.edit("1", None, Some(vec!["new_tag".to_string()])).await?;

// Update both
store.edit(
    "1",
    Some("New content".to_string()),
    Some(vec!["tag1".to_string(), "tag2".to_string()])
).await?;
```

---

### Pagination

```rust
// Page 1: items 0-19
let page1 = store.list(20, 0).await?;

// Page 2: items 20-39
let page2 = store.list(20, 20).await?;

// Total count for UI pagination
let total = store.count().await?;
```

---

## Testing

```bash
# Run all memory tool tests
cargo test -p operon-tools-memory

# Run store tests only
cargo test -p operon-tools-memory-store

# Run specific tool tests
cargo test -p operon-tools-memory-add
cargo test -p operon-tools-memory-search
```

---

## Migration from Session-Scoped Todos

```mermaid
graph TB
    subgraph "Session-Scoped (todos)"
        A1[In-memory Vec] --> A2[Cleared on restart]
        A2 --> A3[Scoped to session]
    end
    
    subgraph "Global Persistent (memories)"
        B1[SQLite WAL] --> B2[Survives restarts]
        B2 --> B3[Shared across sessions]
    end
    
    style A1 fill:#FFD700
    style B1 fill:#90EE90
```

| Aspect | Todos | Memories |
|--------|-------|----------|
| **Scope** | Session | Global |
| **Persistence** | In-memory | SQLite |
| **Lifetime** | Until session ends | Indefinite |
| **Search** | Linear scan | FTS5 indexed |
| **Use Case** | Task tracking | Facts, preferences |

---

## Dependencies

```toml
# Facade crate
[dependencies]
operon-tools-memory-add        = { workspace = true }
operon-tools-memory-edit       = { workspace = true }
operon-tools-memory-delete     = { workspace = true }
operon-tools-memory-retrieve   = { workspace = true }
operon-tools-memory-search     = { workspace = true }
operon-tools-core              = { workspace = true }
serde                          = { workspace = true }
serde_json                     = { workspace = true }
tokio                          = { workspace = true }
operon-context-normalize-tools = { workspace = true }
```

```toml
# Store crate (leaf)
[dependencies]
thiserror     = { workspace = true }
serde         = { workspace = true }
serde_json    = { workspace = true }
tokio         = { workspace = true }
chrono        = { workspace = true }
operon-config = { workspace = true }
sqlx          = { workspace = true, features = ["sqlite", "runtime-tokio", "macros"] }
```

---

## Design Rationale

### Why SQLite Over JSON Files?

```mermaid
flowchart LR
    A[SQLite] --> B[ACID transactions]
    A --> C[FTS5 indexed search]
    A --> D[Concurrent access]
    A --> E[Automatic triggers]
    
    F[JSON Files] --> G[Manual locking]
    F --> H[Linear scan search]
    F --> I[Race conditions]
    F --> J[Manual sync logic]
    
    style A fill:#90EE90
    style F fill:#FF6B6B
```

---

### Why FTS5 Over LIKE Queries?

```sql
-- LIKE query (slow)
SELECT * FROM memories WHERE content LIKE '%dark mode%';

-- FTS5 query (fast)
SELECT * FROM memories m
INNER JOIN memories_fts ON memories_fts.rowid = m.id
WHERE memories_fts MATCH 'dark mode'
ORDER BY rank;
```

**FTS5 Advantages**:
- Tokenized indexing (handles word boundaries correctly)
- BM25 relevance ranking
- Phrase matching, Boolean operators
- Sub-millisecond search even with thousands of memories

---

### Why No Separate Tags Table?

**Option A** (Rejected):
```sql
CREATE TABLE memories (...);
CREATE TABLE tags (id, name);
CREATE TABLE memory_tags (memory_id, tag_id);
```

**Complexity**: 3 tables, JOINs, cascading deletes

**Option B** (Chosen):
```sql
CREATE TABLE memories (..., tags TEXT);  -- JSON array
```

**Simplicity**: Single table, no JOINs, deserialize in Rust

---

## License

Operon is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

---

> Built by **Luka Gray (aka Soumo Mukherjee)** • West Bengal, India • 2026
