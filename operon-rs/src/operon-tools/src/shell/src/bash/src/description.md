Executes a command in a new shell subprocess at the specified working directory.

Format:
<bash path="[working_directory]">
<<<<
command="[shell_command]"
timeout="[milliseconds]"
>>>>

Constraints & Usage:
- `path` must be an absolute path to an existing directory within allowed boundaries.
- Subprocesses are stateless; environment variables and directory changes (`cd`) do not persist between separate tool calls. Combine commands using `&&` or `;` within a single call.
- `command` (required): The shell command to run.
- `timeout` (optional): Kill command after specified milliseconds (default: 30 minutes).
- Output is merged stdout/stderr (capped at 10,000 characters).
