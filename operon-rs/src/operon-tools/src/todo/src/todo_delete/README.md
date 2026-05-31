# operon-tools-todo-delete

Deletes a todo item from the agent's task list by ID.

## Overview

The `todo_delete` tool removes a todo item from the task list. Prefer marking items "completed" over deleting them — deletion is for items added by mistake. Completed items should be kept for task history visibility.

## Usage

```rust
use operon_tools_todo_delete::{definition, execute};
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
            "id": item.id
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
      "description": "Id of the todo item to delete."
    }
  },
  "required": ["id"]
}
```

## Output

Returns a JSON object with:
- `id`: The ID that was deleted
- `remaining`: Total number of todos remaining after deletion

## When to Delete vs Mark Completed

- **Mark completed**: Use `todo_update` with `status: "completed"` for tasks that were done. This preserves task history and visibility into what was accomplished.
- **Delete**: Use `todo_delete` only for items added by mistake or that are no longer relevant. Deletion removes the item entirely.

## Error Cases

- **ID not found**: Returns error with message "todo not found: id 'X'"
- **Malformed JSON**: Returns `Err(TodoDeleteToolError::ArgsParse)` if arguments don't match schema

## Examples

### Delete an Item Added by Mistake
```json
{
  "id": "3"
}
```

### Response (Success)
```json
{
  "id": "3",
  "remaining": 2
}
```

### Response (Error - Not Found)
```json
{
  "is_error": true,
  "content": "todo not found: id '99999'"
}
```

## Workflow Guidance

1. **Create tasks** at the start of work using `todo_create`
2. **Mark in_progress** when starting work using `todo_update`
3. **Mark completed** when done using `todo_update` (preserves history)
4. **Delete only** if added by mistake using `todo_delete`

This workflow maintains a complete record of work accomplished while keeping the active task list clean.

## Integration

This tool is registered with the dispatcher and requires mutable access to the `TodoStore`. It's routed directly in the dispatcher's `dispatch()` method before generic tool lookup.

## Testing

Comprehensive tests are provided in `src/tests.rs` covering:
- Basic deletion
- Correct ID returned
- Remaining count decrements
- Multiple sequential deletions
- Persistence verification
- Selective deletion (leaves other items intact)
- Nonexistent ID error handling
- Failed delete doesn't modify store
- Malformed JSON validation
- Edge cases (empty store, delete twice)
