# operon-tools-load

The `load_tools` tool for the Operon agent. Returns tool names, short descriptions, and JSON schemas for a named built-in tool group on demand.

## Overview

`load_tools` is a bootstrap tool that enables on-demand discovery of tool definitions. Instead of loading all tool definitions upfront (which would bloat every request), the model calls `load_tools` to discover which tools are available in a group before using them.

## Two Call Modes

### 1. List All Groups (No Arguments)

Call `load_tools` with no `group` argument to see all available tool groups:

```json
{
  "available_groups": ["fs", "shell", "web", "todo"],
  "message": "Call load_tools with a group name to load its tools. Available: fs, shell, web, todo"
}
```

### 2. Load a Specific Group (With Group Argument)

Call `load_tools { group: "fs" }` to load all tools in the "fs" group:

```json
{
  "group": "fs",
  "tool_count": 7,
  "tools": [
    {
      "name": "read",
      "description": "Reads one or multiple files...",
      "parameters": { "type": "object", "properties": { ... } }
    },
    ...
  ]
}
```

## Why On-Demand Loading?

Tools are not available until explicitly loaded. This design keeps context efficient:

- **Without on-demand loading**: Every request would include definitions for 20+ tools, consuming significant tokens.
- **With on-demand loading**: The model loads only the groups it needs, when it needs them.

## Workflow Example

1. **Discover groups**: Call `load_tools {}` → see available groups
2. **Load a group**: Call `load_tools { group: "fs" }` → see fs tools
3. **Use tools**: Use fs tools (read, write, grep, etc.) with confidence

## Error Handling

If you pass an unknown group name:

```
unknown group: 'xyz'. Call load_tools with no arguments to list available groups.
```

## Implementation Details

- **Location**: `operon-rs/src/operon-tools/src/load/`
- **Crate**: `operon-tools-load`
- **Group**: `"core"` (internal, not user-loadable)
- **Dispatch**: Intercepted directly in `Dispatcher::dispatch()` before generic tool lookup
- **Access**: Always available without loading (it's the bootstrap tool)

## For Extensions

For installed extensions (OHub), use `mcp_load` instead of `load_tools`. The `load_tools` tool is for built-in groups only.

## Architecture

The tool is split across three components:

1. **Dispatcher** (`operon-rs/src/operon-tools/src/dispatcher.rs`):
   - Intercepts `load_tools` calls before generic dispatch
   - Extracts group definitions via `definitions_for_group()`
   - Extracts all groups via `registered_groups()`
   - Passes data to load_tools executor

2. **load_tools Crate** (`operon-rs/src/operon-tools/src/load/`):
   - `definition()`: Returns tiered tool definition (short + detailed)
   - `execute_with_defs()`: Formats group tools for response
   - `execute_list_groups()`: Formats all groups for response

3. **Snapshot** (`operon-rs/src/operon-context/src/operon-context-snapshot/`):
   - `tool_groups` block: Lists available groups in system message
   - Tells the model which groups exist and how to load them
   - Populated from `SnapshotConfig::tool_groups`

## Testing

Run tests with:

```bash
cargo test -p operon-tools-load
```

Tests cover:
- Loading a specific group returns correct tools
- Unknown group returns error
- Listing all groups returns all available groups
- Empty group list is handled gracefully
