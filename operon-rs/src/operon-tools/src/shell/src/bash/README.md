# operon-tools-shell-bash

Bash tool: executes a shell command in a stateless subprocess, captures merged stdout+stderr, returns exit code and output. Output capped at 10,000 characters. Optional model-specified timeout.

## Overview

The `bash` tool allows the Operon agent to execute arbitrary shell commands and retrieve their output. Each execution is **stateless** — no environment variables, working directory changes, or shell state persists between calls. Commands run in a fresh subprocess each time.

### Key Features

- **Stateless execution**: Each call spawns a fresh `sh -c` subprocess (Unix) or `cmd /C` (Windows)
- **Merged output**: Stdout and stderr are captured and merged in order
- **Output truncation**: Output is capped at 10,000 characters to prevent context bloat
- **Optional timeout**: Specify `timeout_ms` to kill long-running commands
- **Exit code tracking**: Returns the process exit code (0 = success, non-zero = failure, -1 = timeout)
- **Cross-platform**: Works on Unix/Linux, macOS, and Windows

## Usage

### Basic Example

```rust
use operon_tools_shell_bash::{definition, execute};
use operon_context_normalize_tools::ToolCallId;
use serde_json::json;

#[tokio::main]
async fn main() {
    // Get the tool definition for registration
    let def = definition();

    // Execute a command
    let result = execute(
        ToolCallId("call_1".to_string()),
        json!({
            "command": "echo hello"
        })
    ).await.unwrap();

    println!("{:?}", result);
}
```

### Input Schema

```json
{
  "command": "string (required)",
  "timeout_ms": "integer (optional, milliseconds)"
}
```

- **`command`** (required): Shell command to execute. Runs in a fresh `sh -c` subprocess on Unix, `cmd /C` on Windows.
- **`timeout_ms`** (optional): Timeout in milliseconds. If omitted, the command runs until completion.

### Output Schema

Success and timeout cases return JSON:

```json
{
  "command": "string",
  "exit_code": "integer",
  "output": "string",
  "truncated": "boolean",
  "timed_out": "boolean"
}
```

- **`command`**: The command that was executed (echoed back for correlation)
- **`exit_code`**: Exit code of the process (0 = success, non-zero = failure, -1 = timeout)
- **`output`**: Merged stdout + stderr, truncated to 10,000 characters
- **`truncated`**: True if the output was truncated
- **`timed_out`**: True if the command was killed due to timeout

Failure cases (empty command, spawn failure) return Text:

```
"command is empty"
"failed to spawn process: ..."
```

## Stateless Execution Model

Each call is independent. Working directory, environment variables, shell variables, and `cd` changes do **NOT** persist between calls.

### ❌ Incorrect: Expecting cd to persist

```json
{
  "command": "cd /tmp"
}
```

Then in a separate call:

```json
{
  "command": "pwd"
}
```

Result: `pwd` returns the original working directory, NOT `/tmp`. The `cd` from the first call did not persist.

### ✓ Correct: Chain commands in one call

```json
{
  "command": "cd /tmp && pwd"
}
```

Result: `pwd` returns `/tmp` because both commands run in the same subprocess.

## Output Truncation

Stdout and stderr are merged and truncated to 10,000 characters. When `truncated: true`, use more targeted commands to retrieve the specific part of the output needed:

- `| head -n 50` — first 50 lines
- `| tail -n 20` — last 20 lines
- `| grep "pattern"` — lines matching a pattern

### Example: Large Output

```json
{
  "command": "python3 -c \"print('a' * 20000)\""
}
```

Result: `output` contains the first 10,000 characters, `truncated: true`.

To get the last part:

```json
{
  "command": "python3 -c \"print('a' * 20000)\" | tail -c 1000"
}
```

## Exit Codes

- **`exit_code: 0`** — command succeeded
- **`exit_code: N`** (non-zero) — command reported failure. The command ran — the model receives the output and decides what to do next. Non-zero exit is **NOT** a tool error.
- **`exit_code: -1`** — process was killed due to timeout (see `timed_out: true`)

Always check `exit_code` before treating output as valid.

## Timeout Behavior

When `timed_out: true`, the process was killed and `output` contains whatever was buffered before the kill. `exit_code` will be -1.

### Example: Timeout

```json
{
  "command": "sleep 10",
  "timeout_ms": 200
}
```

Result: `timed_out: true`, `exit_code: -1`, `output` is empty (sleep produces no output).

## No Timeout by Default

If `timeout_ms` is omitted, the command runs until completion. Use this for commands with unpredictable duration (builds, package installs). Set a timeout when you need a hard deadline.

## When to Use bash vs fs Tools

Prefer fs tools (`read`, `edit`, `write`, `grep`, `ls`) for file operations — they are faster, safer, and return structured output.

Use `bash` for:

- Running build systems (make, cargo, npm, etc.)
- Package managers (apt, pip, npm install, etc.)
- Git operations (git clone, git commit, etc.)
- Test runners (pytest, cargo test, npm test, etc.)
- CLI tools and utilities
- Anything requiring shell features (pipes, environment variables, process management)

## Common Mistakes

### Mistake #1: Expecting cd to persist

```json
{
  "command": "cd /tmp"
}
```

Then later:

```json
{
  "command": "pwd"
}
```

Result: `pwd` returns the original directory, NOT `/tmp`. Each call is stateless.

**Fix**: Use `cd /tmp && pwd` in a single call.

### Mistake #2: Running a command that produces massive output without piping

```json
{
  "command": "cat /var/log/huge_file.log"
}
```

Result: `output` is truncated to 10,000 characters. Important lines may be lost.

**Fix**: Use `| head -n 50` or `| tail -n 50` to get the specific part you need.

### Mistake #3: Forgetting to set timeout for long-running commands

```json
{
  "command": "npm install"
}
```

Result: The command may take minutes. If the model's context window expires, the call is abandoned.

**Fix**: Set `timeout_ms` to a reasonable value for the expected duration, or accept that long commands may not complete.

### Mistake #4: Empty command

```json
{
  "command": ""
}
```

Error: `"command is empty"`

**Fix**: Provide a non-empty command.

## Error Messages

- `"command is empty"` → Provide a non-empty command.
- `"failed to spawn process: ..."` → OS-level error (permission denied, command not found, etc.). The command was not executed.

## Implementation Details

### Architecture

The bash tool follows the standard Operon tool structure:

- **`args.rs`**: Argument deserialization (`BashArgs`)
- **`output.rs`**: Output structure (`BashOutput`)
- **`error.rs`**: Error types (`BashToolError`)
- **`executor.rs`**: Core execution logic
- **`lib.rs`**: Public API (`definition()`, `execute()`)
- **`tests.rs`**: Comprehensive test suite

### Executor Logic

1. **Validate command** is non-empty
2. **Spawn subprocess** with `sh -c` (Unix) or `cmd /C` (Windows)
3. **Capture stdout and stderr** concurrently (prevents deadlock)
4. **Apply timeout** if specified
5. **Merge and truncate** output to 10,000 characters
6. **Return result** with exit code and metadata

### Key Design Decisions

- **Stateless by design**: Each call spawns a fresh subprocess. No state persists.
- **Merged output**: Stdout and stderr are merged in order (same as `2>&1` in shells).
- **Concurrent I/O**: Stdout and stderr are read concurrently using `tokio::join!` to prevent deadlock.
- **Timeout support**: Optional timeout via `tokio::time::timeout`.
- **Output truncation**: Truncated at character boundary (not byte boundary) to preserve UTF-8 validity.
- **Non-error non-zero exits**: Non-zero exit codes are NOT tool errors. The model receives the output and decides.

## Integration

The bash tool is registered in the Operon dispatcher via `register_shell_tools()`:

```rust
use operon_tools::dispatcher::Dispatcher;

let mut dispatcher = Dispatcher::new();
dispatcher.register_fs_tools();
dispatcher.register_shell_tools();  // Registers bash + other shell tools

// Get definitions to send to the model
let defs: Vec<_> = dispatcher.definitions().collect();
```

## Testing

The bash tool includes comprehensive tests covering:

- **Success cases**: Basic command, non-zero exit, stderr capture, output merging, command chaining
- **Timeout cases**: Timeout kills command, partial output captured
- **Truncation**: Output truncated at 10,000 characters
- **Stateless execution**: cd doesn't persist between calls
- **Failure cases**: Empty command, malformed args
- **Edge cases**: Whitespace-only command, command echoed in output

Run tests with:

```bash
cargo test -p operon-tools-shell-bash
```

All tests pass with zero warnings.

## Code Quality

- **No unsafe code**: All code is safe Rust
- **No unwrap() in executor**: Only `expect()` on guaranteed-safe operations (stdout/stderr `.take()`)
- **Full documentation**: Every public item has doc comments
- **Module-level docs**: Every file has module-level documentation
- **Clippy clean**: No clippy warnings
- **Cross-platform**: Works on Unix/Linux, macOS, and Windows

## Performance

- **Async I/O**: Uses `tokio::process` for non-blocking subprocess execution
- **Concurrent output capture**: Stdout and stderr are read concurrently to prevent deadlock
- **Efficient truncation**: Truncation is done at character boundary without allocating intermediate strings
- **No temp files**: No temporary files or extra allocations needed

## Limitations

- **Output cap**: Output is truncated to 10,000 characters. Use piping to retrieve specific parts.
- **Stateless**: No state persists between calls. Chain commands with `&&` or `;` for sequential state.
- **No shell features**: Commands run in `sh -c` (Unix) or `cmd /C` (Windows). Some shell features may not be available.
- **Timeout precision**: Timeout is approximate and depends on system load.
