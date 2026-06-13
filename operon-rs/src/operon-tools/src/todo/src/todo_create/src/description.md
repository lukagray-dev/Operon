Creates a new task in the session's todo list.

Format:
<todo_create todo="[task_description]" priority="[high_medium_low]">

Constraints & Usage:
- `todo` (required): A descriptive task description (e.g., "Implement search logic").
- `priority` (optional): Set to `"high"`, `"medium"`, or `"low"` (default is `"medium"`).
- Newly created tasks start with status `"pending"`.
