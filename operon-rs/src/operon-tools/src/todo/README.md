# operon-tools-todo

Facade crate for all todo tools: create, list, update, delete.

## Overview

The todo tools implement a session-scoped task list for the agent to plan and track work. All four tools operate on the same in-memory `TodoStore` owned by the `Dispatcher`. Todos are session-scoped — they exist for the duration of the agent session only and do not persist across sessions.

## Architecture

```
operon-tools-todo (facade)
├── operon-tools-todo-create  (creates new items)
├── operon-tools-todo-list    (lists items with filtering)
├── operon-tools-todo-update  (updates existing items)
└── operon-tools-todo-delete  (deletes items)
```

All tools share:
- **TodoItem** type: id, content, status, priority
- **TodoStore**: in-memory session-scoped storage
- **Dispatcher routing**: direct dispatch before generic tool lookup

## Quick Start

### Register with Dispatcher

```rust
use operon_tools::dispatcher::Dispatcher;

let mut dispatcher = Dispatcher::new();
dispatcher.register_fs_tools();
dispatcher.register_shell_tools();
dispatcher.register_todo_tools();  // Add this

// Get definitions to send to the model
let defs: Vec<_> = dispatcher.definitions().collect();
```

### Create a Todo

```json
{
  "name": "todo_create",
  "arguments": {
    "content": "Fix the login bug",
    "priority": "high"
  }
}
```

### List Todos

```json
{
  "name": "todo_list",
  "arguments": {
    "status": "pending"
  }
}
```

### Update a Todo

```json
{
  "name": "todo_update",
  "arguments": {
    "id": "1",
    "status": "in_progress"
  }
}
```

### Delete a Todo

```json
{
  "name": "todo_delete",
  "arguments": {
    "id": "1"
  }
}
```

## Workflow Example

```rust
use operon_tools_todo_create;
use operon_tools_todo_list;
use operon_tools_todo_update;
use operon_tools_todo_delete;
use operon_tools_core::TodoStore;
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    let mut store = TodoStore::new();
    
    // 1. Create tasks
    let result1 = todo_create::execute(
        ToolCallId("1".to_string()),
        json!({"content": "Task 1", "priority": "high"}),
        &mut store
    ).await.unwrap();
    let item1_id = /* extract from result */;
    
    // 2. List all tasks
    let result = todo_list::execute(
        ToolCallId("2".to_string()),
        json!({}),
        &store
    ).await.unwrap();
    
    // 3. Mark task in progress
    todo_update::execute(
        ToolCallId("3".to_string()),
        json!({"id": item1_id, "status": "in_progress"}),
        &mut store
    ).await.unwrap();
    
    // 4. Mark task completed
    todo_update::execute(
        ToolCallId("4".to_string()),
        json!({"id": item1_id, "status": "completed"}),
        &mut store
    ).await.unwrap();
    
    // 5. Delete if needed (prefer marking completed)
    // todo_delete::execute(...).await.unwrap();
}
```

## Data Model

### TodoItem

```rust
pub struct TodoItem {
    pub id: String,              // Auto-assigned: "1", "2", "3", ...
    pub content: String,         // Task description (imperative form)
    pub status: TodoStatus,      // Pending, InProgress, Completed
    pub priority: TodoPriority,  // High, Medium, Low
}
```

### TodoStatus

- **Pending**: Task has not been started yet (default on creation)
- **InProgress**: Task is currently being worked on
- **Completed**: Task has been completed

### TodoPriority

- **High**: Urgent, should be done first
- **Medium**: Normal priority (default on creation)
- **Low**: Can be deferred

## Session Scope

Todos are **session-scoped**:
- Created when the agent session starts
- Exist for the duration of the session
- Lost when the session ends
- **NOT** persisted to disk or database
- **NOT** cleared by context compaction (task plan survives summarization)

## Tool Descriptions

### todo_create

Creates a new todo item with auto-assigned ID.

- **Input**: `content` (required), `priority` (optional)
- **Output**: Created item with ID and total count
- **Defaults**: Status=Pending, Priority=Medium

See [todo_create/README.md](src/todo_create/README.md) for details.

### todo_list

Lists todos with optional filtering by status or priority.

- **Input**: `status` (optional), `priority` (optional)
- **Output**: Filtered items, total count, and status counts
- **Filtering**: Both filters optional, can be combined
- **Counts**: Always from full unfiltered list

See [todo_list/README.md](src/todo_list/README.md) for details.

### todo_update

Updates existing todo items with partial update semantics.

- **Input**: `id` (required), `content`, `status`, `priority` (all optional)
- **Output**: Updated item
- **Semantics**: Only provided fields change
- **Validation**: At least one field must be provided

See [todo_update/README.md](src/todo_update/README.md) for details.

### todo_delete

Deletes a todo item by ID.

- **Input**: `id` (required)
- **Output**: Deleted ID and remaining count
- **Guidance**: Prefer marking completed over deleting

See [todo_delete/README.md](src/todo_delete/README.md) for details.

## Dispatcher Integration

Todo tools are **stateful** and require mutable access to `TodoStore`. They are routed directly in the dispatcher's `dispatch()` method before generic tool lookup:

```rust
match call.name.as_str() {
    "todo_create" => {
        return operon_tools_todo_create::execute(
            call.id,
            call.arguments,
            &mut self.todo_store,
        ).await.unwrap_or_else(|e| error_result(...));
    }
    // ... other todo tools ...
    _ => {} // fall through to generic tool dispatch
}
```

## Testing

Each tool sub-crate includes comprehensive tests in `src/tests.rs`:

- **todo_create**: 10 tests (creation, validation, ID uniqueness)
- **todo_list**: 13 tests (listing, filtering, counts)
- **todo_update**: 15 tests (updates, partial semantics, validation)
- **todo_delete**: 11 tests (deletion, error handling, edge cases)

Run tests with:
```bash
cargo test -p operon-tools-todo-create
cargo test -p operon-tools-todo-list
cargo test -p operon-tools-todo-update
cargo test -p operon-tools-todo-delete
```

## Best Practices

1. **Create a todo list at the start of multi-step tasks** to plan your work
2. **Use imperative form** for task descriptions: "Fix bug" not "Bug fix"
3. **Set priority appropriately**: High for urgent, Medium for normal, Low for deferred
4. **Mark in_progress when starting work** to show active tasks
5. **Mark completed when done** to preserve task history
6. **Delete only for mistakes** — prefer marking completed for visibility
7. **Check the list regularly** to stay on track with `todo_list`

## Dependencies

- `tokio`: Async runtime
- `serde`/`serde_json`: Serialization
- `thiserror`: Error types
- `operon-context-normalize-tools`: Tool infrastructure
- `operon-tools-core`: Shared types (TodoItem, TodoStore, etc.)

## License

AGPL-3.0
