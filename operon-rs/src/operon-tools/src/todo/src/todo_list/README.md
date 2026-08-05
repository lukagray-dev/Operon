# operon-tools-todo-list

Lists all todo items in the agent's task list with optional filtering by status or priority.

## Overview

The `todo_list` tool returns the current task list with optional filtering. It always includes status counts (pending, in_progress, completed) for quick overview of work progress, even when filters are applied.

## Usage

```rust
use operon_tools_todo_list::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::TodoStore;
use serde_json::json;

#[tokio::main]
async fn main() {
    let store = TodoStore::new();
    
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "status": "pending"
        }),
        &store
    ).await.unwrap();
    
    println!("{:?}", result);
}
```

## Input Schema

```json
{
  "type": "object",
  "properties": {
    "status": {
      "type": "string",
      "enum": ["pending", "in_progress", "completed"],
      "description": "Optional filter by status."
    },
    "priority": {
      "type": "string",
      "enum": ["high", "medium", "low"],
      "description": "Optional filter by priority."
    }
  }
}
```

## Output

Returns a JSON object with:
- `items`: Array of `TodoItem` objects matching the filters (or all items if no filters)
- `total`: Total number of todos in the store (unfiltered count)
- `pending`: Count of items with status "pending" (unfiltered)
- `in_progress`: Count of items with status "in_progress" (unfiltered)
- `completed`: Count of items with status "completed" (unfiltered)

## Filtering

Both filters are optional and can be combined:
- **No filters**: Returns all items
- **Status filter only**: Returns items matching the status
- **Priority filter only**: Returns items matching the priority
- **Both filters**: Returns items matching both status AND priority

## Status Counts

The status counts are **always computed from the full unfiltered list**, giving you a complete overview of work progress even when filtering. This allows you to see the big picture while focusing on specific items.

## Error Cases

- **Invalid status value**: Returns `Err(TodoListToolError::ArgsParse)` if status is not a valid enum value
- **Invalid priority value**: Returns `Err(TodoListToolError::ArgsParse)` if priority is not a valid enum value
- **Empty list**: Returns success with empty items array (not an error)

## Examples

### List All Items
```json
{}
```

### List Pending Items
```json
{
  "status": "pending"
}
```

### List High-Priority Items
```json
{
  "priority": "high"
}
```

### List High-Priority Pending Items
```json
{
  "status": "pending",
  "priority": "high"
}
```

### Response (Success)
```json
{
  "items": [
    {
      "id": "1",
      "content": "Fix the login bug",
      "status": "pending",
      "priority": "high"
    }
  ],
  "total": 3,
  "pending": 2,
  "in_progress": 1,
  "completed": 0
}
```

## Integration

This tool is registered with the dispatcher and requires immutable access to the `TodoStore`. It's routed directly in the dispatcher's `dispatch()` method before generic tool lookup.

## Testing

Comprehensive tests are provided in `src/tests.rs` covering:
- Listing all items
- Empty list handling
- Filtering by status (pending, in_progress, completed)
- Filtering by priority (high, medium, low)
- Combined status and priority filters
- Status count verification
- Counts unaffected by filters
- Invalid status/priority validation
