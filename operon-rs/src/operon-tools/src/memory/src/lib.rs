//! # operon-tools-memory
//!
//! This is the shared library crate for all Operon memory tools.
//! It handles the database connection setup, directories verification,
//! and schema creation for the global SQLite database used by the agent.
//!
//! By putting this in a central crate, we avoid duplicate database connection code
//! across `memory_add`, `memory_delete`, `memory_edit`, `memory_retrieve`, and `memory_search`.

use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::Connection;
use std::path::PathBuf;
use std::cell::RefCell;

thread_local! {
    // Holds a thread-local database file path to support isolated test databases per test thread.
    static TEST_DB_PATH: RefCell<Option<PathBuf>> = RefCell::new(None);
}

/// Sets a thread-local database path to isolate SQLite databases across concurrent tests.
pub fn set_test_db_path(path: PathBuf) {
    TEST_DB_PATH.with(|p| {
        *p.borrow_mut() = Some(path);
    });
}

/// Clears the thread-local database path override.
pub fn clear_test_db_path() {
    TEST_DB_PATH.with(|p| {
        *p.borrow_mut() = None;
    });
}

/// Resolves the absolute database file path on the system.
///
/// Operon stores all session files and configurations in the `~/.operon/` directory.
/// We store the global memory database at `~/.operon/memory/memory.db`.
/// If the home directory cannot be resolved, it falls back to the current directory.
pub fn get_db_path() -> PathBuf {
    // dirs::home_dir() finds the appropriate home folder based on the OS:
    // Windows: C:\Users\<username>
    // macOS: /Users/<username>
    // Linux: /home/<username>
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
        .join(".operon")
        .join("memory")
        .join("memory.db")
}

/// Connects to the SQLite database and ensures the memories table is initialized.
///
/// In tests, this will automatically connect to a temporary in-memory database (`:memory:`)
/// so that tests are isolated, run in parallel, and don't write garbage to the user's home folder.
pub async fn connect_db() -> Result<SqliteConnection, sqlx::Error> {
    // Check if a custom thread-local database path has been configured for testing.
    let test_path = TEST_DB_PATH.with(|p| p.borrow().clone());

    let options = if let Some(db_path) = test_path {
        // If a thread-local path is set, use it. Ensure its parent directories exist.
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
    } else {
        // When running unit or integration tests of this crate itself, use a temporary SQLite file.
        #[cfg(test)]
        let db_path = std::env::temp_dir().join("operon_test_memory.db");

        // In production or development mode, use the persistent file in the home directory.
        #[cfg(not(test))]
        let db_path = get_db_path();
        
        // Ensure the directory `~/.operon/memory/` exists on disk before connecting.
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        
        SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
    };

    // Open/connect to the SQLite database.
    let mut conn = SqliteConnection::connect_with(&options).await?;

    // Set up our table schema if it doesn't already exist.
    // Schema fields:
    // - id: unique integer primary key with auto-increment.
    // - content: the raw text memory recorded by the agent.
    // - created_at: when the memory was first written.
    // - updated_at: when the memory was edited.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS memories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            content TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );"
    )
    .execute(&mut conn)
    .await?;

    // Clear the table during unit tests of the memory facade crate itself.
    #[cfg(test)]
    {
        sqlx::query("DELETE FROM memories;").execute(&mut conn).await?;
        sqlx::query("DELETE FROM sqlite_sequence WHERE name = 'memories';").execute(&mut conn).await?;
    }

    Ok(conn)
}

/// Represents a single memory entry stored in the database.
/// This matches the structure sent back and forth between the Tauri Rust backend and JS frontend.
/// Fields are annotated to serialize to camelCase for standard JavaScript conventions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    /// Unique auto-incrementing integer key.
    pub id: i64,
    /// The actual text content of the memory.
    pub content: String,
    /// The timestamp when the memory was first created.
    pub created_at: String,
    /// The timestamp when the memory was last edited or modified.
    pub updated_at: String,
}

/// Retrieves all stored memories from the SQLite database.
/// Returns a list of MemoryItem structs ordered by the `updated_at` timestamp descending (most recent first).
pub async fn get_all_memories() -> Result<Vec<MemoryItem>, sqlx::Error> {
    // Connect to the SQLite database
    let mut conn = connect_db().await?;
    
    // Fetch all columns from the memories table, sorted by last update date
    let rows = sqlx::query(
        "SELECT id, content, created_at, updated_at FROM memories ORDER BY updated_at DESC"
    )
    .fetch_all(&mut conn)
    .await?;

    // Map database rows into structured MemoryItem structs
    let mut items = Vec::new();
    for row in rows {
        use sqlx::Row;
        items.push(MemoryItem {
            id: row.try_get("id")?,
            content: row.try_get("content")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }

    Ok(items)
}

/// Inserts a new memory content into the database and returns the fully populated MemoryItem.
pub async fn add_memory(content: &str) -> Result<MemoryItem, sqlx::Error> {
    // Connect to the SQLite database
    let mut conn = connect_db().await?;
    
    // Insert the text content and immediately return the populated columns (SQLite RETURNING clause)
    let row = sqlx::query(
        "INSERT INTO memories (content) VALUES (?) RETURNING id, content, created_at, updated_at"
    )
    .bind(content)
    .fetch_one(&mut conn)
    .await?;

    use sqlx::Row;
    Ok(MemoryItem {
        id: row.try_get("id")?,
        content: row.try_get("content")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// Updates the text content of a memory entry with a matching ID.
/// Also triggers the `updated_at` column update to track modifications.
pub async fn update_memory(id: i64, content: &str) -> Result<(), sqlx::Error> {
    // Connect to the SQLite database
    let mut conn = connect_db().await?;
    
    // Perform the SQL UPDATE query
    sqlx::query(
        "UPDATE memories SET content = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(content)
    .bind(id)
    .execute(&mut conn)
    .await?;

    Ok(())
}

/// Permanently deletes a memory entry from the database using its unique ID.
pub async fn delete_memory(id: i64) -> Result<(), sqlx::Error> {
    // Connect to the SQLite database
    let mut conn = connect_db().await?;
    
    // Perform the SQL DELETE query
    sqlx::query("DELETE FROM memories WHERE id = ?")
        .bind(id)
        .execute(&mut conn)
        .await?;

    Ok(())
}


