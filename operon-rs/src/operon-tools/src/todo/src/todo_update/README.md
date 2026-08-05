# operon-tools-todo-update

Updates an existing todo item by ID with partial update semantics.

## Overview

The `todo_update` tool allows the agent to modify existing todo items. It supports partial updates — only provided fields are changed, leaving others unchanged. Use this to mark items in_progress as you start work, and completed when done.

## Usage

```rust
use operon_tools_todo_update::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::TodoStore;
use serde_json::json;

#[tokio::main]
async fn main() {
    let mut store = TodoStore::new();
    let item = store.create("Task".to_string(), None);
    
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "id": item.id,
            "status": "in_progress"
        }),
        &mut store
    ).await.unwrap();
    
    println!("{:?}", result);
}
```

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "id": {
      "type": "string",
      "description": "Todo item id."
    },
    "content": {
      "type": "string",
      "minLength": 1,
      "description": "New content. None = no change."
    },
    "status": {
      "type": "string",
      "enum": ["pending", "in_progress", "completed"],
      "description": "New status. None = no change."
    },
    "priority": {
      "type": "string",
      "enum": ["high", "medium", "low"],
      "description": "New priority. None = no change."
    }
  },
  "required": ["id"]
}
```

## Output

Returns a JSON object with:
- `item`: The updated `TodoItem` with all current field values

## Partial Update Semantics

Only provided fields are updated. Fields set to `null` or omitted are not changed:

```json
// Update only status
{
  "id": "1",
  "status": "in_progress"
}

// Update only content
{
  "id": "1",
  "content": "New description"
}

// Update multiple fields
{
  "id": "1",
  "status": "completed",
  "priority": "high"
}
```

## Status Transition Workflow

Recommended workflow for task lifecycle:
1. Create item with status "pending" (default)
2. Update to "in_progress" when starting work
3. Update to "completed" when done

This workflow provides clear visibility into work progress.

## Error Cases

- **ID not found**: Returns error with message "todo not found: id 'X'"
- **No fields to update**: Returns error if only `id` is provided with no other fields
- **Empty content**: Returns error if content is provided but empty or whitespace-only after trimming
- **Invalid status/priority**: Returns `Err(TodoUpdateToolError::ArgsParse)` if values don't match enum
- **Malformed JSON**: Returns `Err(TodoUpdateToolError::ArgsParse)` if arguments don't match schema

## Examples

### Mark Task In Progress
```json
{
  "id": "1",
  "status": "in_progress"
}
```

### Mark Task Completed
```json
{
  "id": "1",
  "status": "completed"
}
```

### Update Content and Priority
```json
{
  "id": "2",
  "content": "Fix the critical login bug",
  "priority": "high"
}
```

### Response (Success)
```json
{
  "item": {
    "id": "1",
    "content": "Fix the login bug",
    "status": "in_progress",
    "priority": "high"
  }
}
```

### Response (Error - No Fields)
```json
{
  "is_error": true,
  "content": "no fields to update — provide at least one of: content, status, priority"
}
```

## Integration

This tool is registered with the dispatcher and requires mutable access to the `TodoStore`. It's routed directly in the dispatcher's `dispatch()` method before generic tool lookup.

## Testing

Comprehensive tests are provided in `src/tests.rs` covering:
- Updating status (to in_progress, to completed)
- Updating content
- Updating priority
- Updating multiple fields
- Whitespace trimming
- Persistence verification
- Nonexistent ID error handling
- No fields error validation
- Empty content validation
- Partial update semantics
