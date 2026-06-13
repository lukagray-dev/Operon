Updates an existing task in the session's todo list.

Format:
<todo_update id="[task_id]" todo="[new_description]" status="[status]" priority="[priority]">

Constraints & Usage:
- `id` (required): The ID of the task to modify.
- Provide at least one field to update:
  * `todo`: Update description.
  * `status`: Update status to `"pending"`, `"in_progress"`, or `"completed"`.
  * `priority`: Update priority to `"high"`, `"medium"`, or `"low"`.
