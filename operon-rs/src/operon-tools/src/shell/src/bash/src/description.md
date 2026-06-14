`bash` tool executes a command in a new shell subprocess at the specified working directory. Use `bash` tool to run shell commands and recive the outputs.

**How to use `bash` tool:**

```example
<bash path="absolute\path\to\directory" command="shell_command" timeout="timeout_in_milliseconds">
```

**Constraints & Usage:**

* **`path`: Must be an absolute path to an existing directory**
* Subprocesses are stateless; environment variables and directory changes (`cd`) do not persist between separate tool calls. Combine commands using `&&` or `;` within a single call.
* **`command`: The shell command to run**
* **`timeout` (optional): Kill command after specified milliseconds (default: 30 minutes)**