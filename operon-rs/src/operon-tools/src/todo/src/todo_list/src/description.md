Lists active todo tasks with optional filters.

Format:
<todo_list status="[status]" priority="[priority]">

Constraints & Usage:
- Filters are optional:
  * `status`: Filter by `"pending"`, `"in_progress"`, or `"completed"`.
  * `priority`: Filter by `"high"`, `"medium"`, or `"low"`.
- Returns the list of matching tasks and summary statistics.
