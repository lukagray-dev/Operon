You are working on the Operon codebase at D:\Project Operon\Operon\operon-rs\

Before writing a single line of code, read the following files to establish full context:

TOOLS (args contracts — understand every tool's expected attrs and body):
- src\operon-tools\src\fs\src\read\src\args.rs
- src\operon-tools\src\fs\src\write\src\args.rs
- src\operon-tools\src\fs\src\append\src\args.rs
- src\operon-tools\src\fs\src\edit\src\args.rs
- src\operon-tools\src\fs\src\delete\src\args.rs
- src\operon-tools\src\fs\src\grep\src\args.rs
- src\operon-tools\src\fs\src\ls\src\args.rs
- src\operon-tools\src\shell\src\bash\src\args.rs

DISPATCHER + SESSION (understand ToolCall shape and how it's consumed):
- src\operon-tools\src\dispatcher.rs
- src\operon-session\src\runner.rs
- src\operon-session\src\http.rs
- src\operon-session\src\request.rs

CONTEXT TYPES (ToolCall, ToolCallId, arguments field type):
- src\operon-context\src\operon-context-normalize\src\operon-context-normalize-tools\src\types.rs

PARSER SKELETON (empty, fill this in):
- src\operon-tools\src\parser\src\lib.rs
- src\operon-tools\src\parser\Cargo.toml

---

## TASK

Implement the `operon-tools-parser` crate at:
  src\operon-tools\src\parser\

This parser converts raw model output text (plain UTF-8 string) into a list of
`RawToolCall` structs, plus the cleaned text with all tag blocks stripped.

The model emits tool calls as XML-style tags with optional bodies, interleaved
with plain prose. Example model output:

  I'll read both files first.

  <read paths="C:\src\main.rs" "C:\src\lib.rs:10-50">

  Now I'll write the new file.

  <write path="C:\src\new.rs">
  <<<<
  fn main() {
      println!("hello");
  }
  >>>>

  Done.

---

## TAG FORMAT SPECIFICATION

### Bodyless tags (no body delimiter):

  <tool_name attr1="value1" attr2="value2">

Single line. Ends at the closing `>`. No body follows. Used by: read, ls, delete,
grep, web_search, web_fetch, todo_*, load_tools, ask.

### Body tags (with body delimiter):
  <tool_name attr1="value1">
  <<<<
  ...raw body content (may contain real newlines, arbitrary text)...
  >>>>

Body starts on the line after `<<<<` and ends on the line BEFORE `>>>>`.
The `<<<<` and `>>>>` are standalone lines (trimmed). Used by: write, append,
edit, bash.

### Determining if a tag has a body:
After the parser sees a complete tag line (ending in `>`), it checks if the
NEXT non-empty line is exactly `<<<<`. If yes — body mode. If no — bodyless.
The parser must NOT buffer ambiguously; it uses a 1-line lookahead approach:
after emitting the tag line, it enters a "waiting for body or not" state until
the next line resolves the question.

---

## ATTR PARSING RULES

Attrs are in the tag line between the tool name and the closing `>`:
  attr1="value1" attr2="value2"

Rules:
1. Values MUST be double-quoted. Unquoted values → reject with error (the tool
   executor will see an ArgsParse error and the model will self-correct).
2. Multiple values for the same key are NOT allowed — last-one-wins would be
   confusing; reject with error.
3. Unknown attrs: silently ignored (forward compatibility).
4. Attr names: ASCII alphanumeric + underscore only. Anything else → ignored.
5. The attr string is the text between the first space after the tool name and
   the closing `>` of the tag.

Parse steps for attr string:
  - Scan left to right
  - Find `key="value"` tokens (key ends at `=`, value starts after `"`, ends
    at the next unescaped `"`)
  - Backslash escapes inside values: `\"` → `"`, `\\` → `\`. No other escapes.
  - After collecting all attrs, produce a flat `HashMap<String, String>`

Body injection:
  - If the tag has a body, inject it into the attr map under the key `"__body__"`
  - The body value is the raw text between `<<<<` and `>>>>`, preserving all
    internal newlines, NOT trimmed (except the leading and trailing newlines
    added by the delimiters themselves — strip exactly one leading newline and
    one trailing newline from the raw captured region)

---

## PARSER STATE MACHINE

States:
  Scanning       — default; scanning for a `<tool_name` opening
  InTag          — inside a tag line (saw `<`, collecting to `>`)
  WaitingBody    — saw complete tag line, next line will determine bodyless vs body
  InBody         — inside `<<<<` ... `>>>>` body block

Transitions:
  Scanning → InTag          when a line contains `<` followed by a valid tool name char
  InTag → WaitingBody       when `>` is found (tag line complete)
  WaitingBody → InBody      when next non-empty trimmed line is exactly `<<<<`
  WaitingBody → Scanning    when next non-empty trimmed line is NOT `<<<<` (emit bodyless call)
  InBody → Scanning         when a trimmed line is exactly `>>>>`  (emit body call)

Text segments (prose between tool calls) are collected separately and included
in the cleaned output.

---

## OUTPUT TYPES

```rust
/// A raw parsed tool call before dispatch.
pub struct RawToolCall {
    /// Tool name extracted from the tag (e.g. "read", "write", "bash").
    pub name: String,

    /// Flat string-keyed attr map. Body tags also have "__body__" key.
    /// Values are already unquoted and backslash-unescaped.
    pub attrs: std::collections::HashMap<String, String>,

    /// Zero-based byte offset of the opening `<` in the original text.
    /// Useful for error reporting and incremental future work.
    pub offset: usize,
}

/// Result of parsing a complete model response text.
pub struct ParseResult {
    /// Tool calls found in the text, in emission order.
    pub calls: Vec<RawToolCall>,

    /// The model's text with all tool-tag blocks (tag line + body delimiters + body)
    /// stripped out. Prose between calls is preserved. Consecutive whitespace-only
    /// lines left by stripping are collapsed to a single blank line.
    pub text: String,
}
```

---

## into_tool_call()

Add a method on `RawToolCall`:

```rust
impl RawToolCall {
    /// Convert into a dispatchable `ToolCall` by assigning a call ID and
    /// moving attrs into a `serde_json::Value` object (all values are strings).
    pub fn into_tool_call(self, call_id: operon_context_normalize_tools::ToolCallId) -> operon_context_normalize_tools::ToolCall {
        let arguments = serde_json::Value::Object(
            self.attrs
                .into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect()
        );
        operon_context_normalize_tools::ToolCall {
            id: call_id,
            name: self.name,
            arguments,
        }
    }
}
```

---

## PUBLIC API

```rust
/// Parse a complete model response string into tool calls + cleaned text.
/// Never panics. Malformed tags produce no call (silently skipped) — the
/// model will see no tool result and can retry on the next turn.
pub fn parse(text: &str) -> ParseResult

/// Parse a complete model response string, returning calls + cleaned text.
/// Same as `parse` but also returns a Vec<ParseError> for diagnostics.
pub fn parse_with_errors(text: &str) -> (ParseResult, Vec<ParseError>)
```

ParseError:
```rust
pub struct ParseError {
    pub offset: usize,
    pub kind: ParseErrorKind,
}

pub enum ParseErrorKind {
    UnquotedAttrValue { key: String },
    DuplicateAttr { key: String },
    UnclosedTag,         // tag line never found its `>`
    UnclosedBody,        // `<<<<` found but `>>>>` never found
    InvalidToolName,     // empty or non-ASCII-alphanumeric-underscore tool name
}
```

---

## Cargo.toml

The crate name is `operon-tools-parser`. Dependencies:
- `serde_json` (workspace)
- `operon-context-normalize-tools` (path, for ToolCall / ToolCallId)

No async. No tokio. Pure sync text processing.

---

## KNOWN TOOL NAMES

The parser is NOT a whitelist — it accepts any tag name that matches
`[a-zA-Z][a-zA-Z0-9_]*`. The dispatcher will return an UnknownTool error for
names it doesn't recognize; the parser's job is only structural.

---

## EDGE CASES TO HANDLE

1. Tool tag spanning multiple chunks (future streaming): not needed now. The
   parser is whole-text only in this phase.
2. `<<<<` or `>>>>` inside a body: `>>>>` always terminates the body — the
   model must not put literal `>>>>` in file content. Document this as a
   known limitation in a comment.
3. Nested tool tags: not supported. If `<` appears inside a body, it is treated
   as body text (not a new tag).
4. Tag with no attrs: valid. `<load_tools>` → name="load_tools", attrs={}.
5. Empty body: valid. `<<<<` immediately followed by `>>>>` → `__body__ = ""`.
6. Prose-only response (no tags): returns ParseResult { calls: vec![], text: original }.
7. Windows paths inside attr values contain `\` — these are NOT escape sequences
   UNLESS `\"` appears. Only `\"` and `\\` are escape sequences; lone `\` is
   kept as-is.

---

## TESTS

Write tests in a `tests.rs` module (mod tests at bottom of lib.rs). Cover:
- Bodyless single call
- Body call (write with multiline content)
- Multiple calls mixed with prose
- Attr with Windows path containing backslashes
- Unquoted attr value → ParseError::UnquotedAttrValue
- Unclosed body → ParseError::UnclosedBody
- Prose-only response → empty calls, text = original
- Empty body (`<<<<` immediately followed by `>>>>`)
- Tag with no attrs (`<load_tools>`)
- into_tool_call() produces correct ToolCall shape

---

## VERIFICATION

After implementation, run:
  cargo build -p operon-tools-parser

Zero errors and zero warnings required.
Do NOT yet wire the parser into dispatcher.rs, runner.rs, or http.rs —
that is a separate step.