# operon-tools-todo-create

Creates a new todo item in the agent's session-scoped task list.

## Overview

The `todo_create` tool allows the agent to add new tasks to its working task list. Each item is assigned a unique auto-incrementing ID and starts with status "pending" and priority "medium" (or as specified).

## Usage

```rust
use operon_tools_todo_create::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use operon_tools_core::TodoStore;
use serde_json::json;

#[tokio::main]
async fn main() {
    let mut store = TodoStore::new();
    
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "content": "Fix the login bug",
            "priority": "high"
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
    "content": {
      "type": "string",
      "minLength": 1,
      "description": "Task description. Use imperative form: 'Implement the grep tool'."
    },
    "priority": {
      "type": "string",
      "enum": ["high", "medium", "low"],
      "description": "Priority level. Default: medium."
    }
  },
  "required": ["content"]
}
```

## Output

Returns a JSON object with:
- `item`: The created `TodoItem` with auto-assigned ID, content, status (pending), and priority
- `total`: Total number of todos in the store after creation

## Error Cases

- **Empty content**: Returns error if content is empty or whitespace-only after trimming
- **Malformed JSON**: Returns `Err(TodoCreateToolError::ArgsParse)` if arguments don't match schema

## Defaults

- **Status**: Always `pending` on creation
- **Priority**: `medium` if not specified

## Example

### Request
```json
{
  "content": "Implement the grep tool",
  "priority": "high"
}
```

### Response (Success)
```json
{
  "item": {
    "id": "1",
    "content": "Implement the grep tool",
    "status": "pending",
    "priority": "high"
  },
  "total": 1
}
```

### Response (Error - Empty Content)
```json
{
  "is_error": true,
  "content": "content is empty"
}
```

## Integration

This tool is registered with the dispatcher and requires mutable access to the `TodoStore`. It's routed directly in the dispatcher's `dispatch()` method before generic tool lookup.

## Testing

Comprehensive tests are provided in `src/tests.rs` covering:
- Basic creation with defaults
- Creation with custom priority
- Whitespace trimming
- Total count verification
- Empty content validation
- ID uniqueness and sequential increment
