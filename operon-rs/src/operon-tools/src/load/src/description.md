Loads and displays tool definitions for a specific group of tools.

Format:

```example
<load_tools>
<<<<
group="group_name"
>>>>
```

Constraints & Usage:

- Call without a group parameter to list all registered tool groups.
- Specify `group` (e.g., `fs`, `shell`, `web`, `todo`, `ask`) to load and register the tools in that group.
- Newly loaded tools become available for use on all subsequent turns.
