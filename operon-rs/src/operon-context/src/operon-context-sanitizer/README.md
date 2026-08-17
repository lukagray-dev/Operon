# operon-context-sanitizer

**Six-stage message array sanitization pipeline for LLM-ready conversation histories**

`operon-context-sanitizer` ensures conversation messages are clean, well-formed, and compliant with LLM provider requirements before every API call. Pure, synchronous, side-effect-free transformation.

---

## Overview

Before each LLM request, the message array must be sanitized to handle:
- Stale system messages from previous snapshots
- Missing per-turn metadata (timestamp, role)
- Orphaned tool calls/results from failed operations
- Malformed tool call fields (missing IDs, JSON-string arguments)
- Out-of-order tool results, role violations, duplicate tool call IDs

```mermaid
flowchart LR
    A[Raw Messages] --> B[Stage 1:<br/>System]
    B --> C[Stage 2:<br/>Metadata]
    C --> D[Stage 3:<br/>Drop Orphans]
    D --> E[Stage 4:<br/>Normalize]
    E --> F[Stage 5:<br/>Integrity]
    F --> G[Clean Messages]
    
    style A fill:#FF6B6B
    style G fill:#90EE90
    style B fill:#87CEEB
    style C fill:#87CEEB
    style D fill:#87CEEB
    style E fill:#87CEEB
    style F fill:#87CEEB
```

**Single Entry Point**: `sanitize(messages, snapshot, role) -> Result<Vec<ConversationMessage>, SanitizerError>`

---

## Six-Stage Pipeline

### Stage 1: System Message Injection

**Purpose**: Replace all stale system messages with a fresh snapshot render

```mermaid
flowchart TD
    A[Input Messages] --> B{Has system<br/>messages?}
    B -->|Yes| C[Drop all system messages]
    B -->|No| D[Continue]
    C --> D
    D --> E[Insert snapshot.render at index 0]
    E --> F[Output]
    
    style E fill:#90EE90
```

**Implementation**: `system::inject_system(messages, snapshot)`

**Behavior**:
1. Filter out all messages with `role == System`
2. Insert `ConversationMessage::system(snapshot.render())` at index 0
3. System message is **always** the first message

**Why**: System prompt changes across turns (new snapshot blocks, updated AGENTS.md, git status). Old system messages must be replaced to reflect current environment state.

---

### Stage 2: Metadata Injection

**Purpose**: Prepend timestamp and role to the **last user message**

```mermaid
flowchart TD
    A[Messages] --> B{Find last<br/>user message}
    B -->|Not found| C[Return unchanged]
    B -->|Found| D{Has text block?}
    D -->|Yes| E[Prepend to first text block]
    D -->|No| F[Insert new text block at index 0]
    E --> G[Output]
    F --> G
    
    style G fill:#90EE90
```

**Implementation**: `metadata::inject_metadata(messages, role)`

**Format**: `[Time: 2026-08-17T14:32:19Z | Role: Owner]\n{original_text}`

**Example**:

```diff
- User: "Can you debug this function?"
+ User: "[Time: 2026-08-17T14:32:19Z | Role: Owner]\nCan you debug this function?"
```

**Why**: LLM needs temporal context and role awareness for each turn. Only the **last** user message is annotated (the current turn being processed).

---

### Stage 3: Drop Orphans (Two Passes)

**Purpose**: Remove tool calls without matching results and tool results without matching calls

#### Pass 3a: Drop Orphan Tool Results

```mermaid
flowchart TD
    A[Messages] --> B[Collect all assistant<br/>tool_call IDs]
    B --> C[Iterate messages]
    C --> D{Role == Tool?}
    D -->|No| E[Keep message]
    D -->|Yes| F{result.call_id<br/>in collected IDs?}
    F -->|Yes| G[Keep result]
    F -->|No| H[Drop result]
    G --> I{Message<br/>empty?}
    H --> I
    I -->|Yes| J[Drop message]
    I -->|No| E
    E --> K[Output]
    J --> K
    
    style H fill:#FF6B6B
    style J fill:#FF6B6B
```

**Implementation**: `orphans::drop_orphan_tool_results(messages)`

**Example**:

```rust
// BEFORE
[
    Assistant: [ToolCall(id="call_1", name="read_file")],
    Tool: [ToolResult(call_id="call_1", ...)],        // ✅ Kept
    Tool: [ToolResult(call_id="call_999", ...)],      // ❌ Dropped (no matching call)
]

// AFTER
[
    Assistant: [ToolCall(id="call_1", name="read_file")],
    Tool: [ToolResult(call_id="call_1", ...)],
]
```

#### Pass 3b: Drop Orphan Tool Calls

```mermaid
flowchart TD
    A[Messages] --> B[Build suffix_result_ids:<br/>For each message, IDs<br/>appearing after it]
    B --> C[Iterate messages]
    C --> D{Role ==<br/>Assistant?}
    D -->|No| E[Keep message]
    D -->|Yes| F{call.id in<br/>suffix_ids?}
    F -->|Yes| G[Keep call]
    F -->|No| H[Drop call]
    G --> I{Message<br/>empty?}
    H --> I
    I -->|Yes| J[Drop message]
    I -->|No| E
    E --> K[Output]
    J --> K
    
    style H fill:#FF6B6B
    style J fill:#FF6B6B
```

**Implementation**: `orphans::drop_orphan_tool_calls(messages)`

**Example**:

```rust
// BEFORE
[
    Assistant: [ToolCall(id="call_1", name="read_file")],
    Assistant: [ToolCall(id="call_2", name="grep_search")],  // ❌ No result follows
    Tool: [ToolResult(call_id="call_1", ...)],
]

// AFTER
[
    Assistant: [ToolCall(id="call_1", name="read_file")],
    Tool: [ToolResult(call_id="call_1", ...)],
]
```

**Why Suffix Check**: A tool call is only valid if its result appears **after** it in the message array. This handles cases where results are out of order.

---

### Stage 4: Normalize Tool Calls

**Purpose**: Fix malformed assistant tool call fields

```mermaid
flowchart TD
    A[Messages] --> B{Role ==<br/>Assistant?}
    B -->|No| C[Skip]
    B -->|Yes| D[Iterate content blocks]
    D --> E{Block is<br/>ToolCall?}
    E -->|No| F[Skip]
    E -->|Yes| G[Trim whitespace<br/>from name]
    G --> H{ID is empty<br/>or whitespace?}
    H -->|Yes| I[Synthesize ID:<br/>synth_name_position]
    H -->|No| J{Arguments is<br/>JSON string?}
    I --> J
    J -->|Yes| K[Parse to object<br/>if valid]
    J -->|No| L[Keep unchanged]
    K --> L
    L --> M[Next block]
    
    style I fill:#FFD700
    style K fill:#FFD700
```

**Implementation**: `normalize::normalize_tool_calls(messages)`

**Fixes Applied**:

| Issue | Fix | Example |
|-------|-----|---------|
| Missing ID | Synthesize from name + position | `""` → `"synth_read_file_0"` |
| Whitespace in name | Trim | `"  read_file  "` → `"read_file"` |
| JSON-string arguments | Parse to object | `"{\"path\":\"/tmp/a\"}"` → `{"path": "/tmp/a"}` |

**Example**:

```rust
// BEFORE
ToolCall {
    id: "",
    name: "  read_file  ",
    arguments: Value::String("{\"path\":\"/tmp/a.txt\"}")
}

// AFTER
ToolCall {
    id: "synth_read_file_0",
    name: "read_file",
    arguments: Value::Object({"path": "/tmp/a.txt"})
}
```

---

### Stage 5: Enforce Integrity (Three Operations)

**Purpose**: Ensure message order, role alternation, and ID uniqueness

#### Operation 5a: Reorder Tool Results

**Move misplaced tool results to appear immediately after their first tool call**

```mermaid
flowchart TD
    A[Messages] --> B[Build first_tool_call_positions<br/>map: call_id → message_index]
    B --> C[Iterate messages]
    C --> D{Block is<br/>ToolResult?}
    D -->|No| E[Keep in place]
    D -->|Yes| F{Result appears BEFORE<br/>its first tool call?}
    F -->|No| E
    F -->|Yes| G[Remove from current message]
    G --> H[Schedule insertion after<br/>target call's message]
    E --> I[Continue]
    H --> I
    I --> J{More blocks?}
    J -->|Yes| C
    J -->|No| K[Insert scheduled results<br/>as Tool messages]
    K --> L[Output]
    
    style G fill:#FFD700
    style K fill:#90EE90
```

**Implementation**: `integrity::reorder_tool_results(messages)`

**Example**:

```rust
// BEFORE (result appears BEFORE its call)
[
    Tool: [ToolResult(call_id="call_1", ...)],        // ❌ Out of order
    Assistant: [ToolCall(id="call_1", ...)],
]

// AFTER
[
    Assistant: [ToolCall(id="call_1", ...)],
    Tool: [ToolResult(call_id="call_1", ...)],        // ✅ Moved after call
]
```

#### Operation 5b: Merge Adjacent Same-Role Messages

**Combine consecutive messages with the same role (except System)**

```mermaid
flowchart TD
    A[Messages] --> B[Iterate]
    B --> C{Same role as<br/>previous?}
    C -->|No| D[Start new message]
    C -->|Yes| E{Role is<br/>System?}
    E -->|Yes| D
    E -->|No| F[Append content to<br/>previous message]
    F --> G{Both end/start<br/>with text?}
    G -->|Yes| H[Join with newline]
    G -->|No| I[Insert separator]
    H --> J[Continue]
    I --> J
    D --> J
    J --> K{More messages?}
    K -->|Yes| B
    K -->|No| L[Output]
    
    style F fill:#90EE90
```

**Implementation**: `integrity::merge_adjacent_same_role_messages(messages)`

**Example**:

```rust
// BEFORE
[
    User: [Text("Can you help?")],
    User: [Text("I need to debug this")],
]

// AFTER
[
    User: [Text("Can you help?\nI need to debug this")],
]
```

**Separator Rules**:

| Last Block | First Block | Separator |
|------------|-------------|-----------|
| Text | Text | `\n` (joined inline) |
| Text | Non-text | `\n` (append to last text) |
| Non-text | Text | `\n` (prepend to first text) |
| Non-text | Non-text | New Text block `"\n"` |

#### Operation 5c: Deduplicate Tool Call IDs

**Drop duplicate tool calls and their corresponding results**

```mermaid
flowchart TD
    A[Messages] --> B[Iterate assistant messages]
    B --> C{Tool call ID<br/>seen before?}
    C -->|No| D[Add to seen_ids set]
    C -->|Yes| E[Drop tool call]
    E --> F[Track duplicate count]
    D --> G[Continue]
    F --> G
    G --> H{More calls?}
    H -->|Yes| B
    H -->|No| I[Iterate messages<br/>in REVERSE]
    I --> J{Block is<br/>ToolResult?}
    J -->|No| K[Keep]
    J -->|Yes| L{Result call_id<br/>is duplicate?}
    L -->|No| K
    L -->|Yes| M[Drop result]
    M --> N[Decrement duplicate count]
    K --> O[Continue]
    N --> O
    O --> P{More blocks?}
    P -->|Yes| I
    P -->|No| Q[Output]
    
    style E fill:#FF6B6B
    style M fill:#FF6B6B
```

**Implementation**: `integrity::deduplicate_tool_call_ids(messages)`

**Example**:

```rust
// BEFORE
[
    Assistant: [ToolCall(id="dup", name="read_file")],
    Tool: [ToolResult(call_id="dup", ...)],
    Assistant: [ToolCall(id="dup", name="read_file")],  // ❌ Duplicate
    Tool: [ToolResult(call_id="dup", ...)],             // ❌ Dropped
]

// AFTER
[
    Assistant: [ToolCall(id="dup", name="read_file")],
    Tool: [ToolResult(call_id="dup", ...)],
]
```

**Why Reverse Iteration**: Matching results are dropped from the end backward, preserving the **first** occurrence of each tool call and its result.

---

## Complete Pipeline Example

```rust
use operon_context_sanitizer::{sanitize, SanitizerError};
use operon_context_normalize_messages::ConversationMessage;
use operon_context_snapshot::{Role, SessionSnapshot};

// Raw messages with multiple issues
let raw_messages = vec![
    ConversationMessage::system("Old system prompt"),  // Stale
    ConversationMessage::user(vec![
        ContentBlock::Text("Debug this".to_string())   // Missing metadata
    ]),
    ConversationMessage::assistant(vec![
        ContentBlock::ToolCall(ToolCall {
            id: "".to_string(),                        // Missing ID
            name: "  read_file  ".to_string(),         // Whitespace
            arguments: Value::String("{}".to_string()) // JSON string
        })
    ]),
    ConversationMessage::tool(vec![
        ContentBlock::ToolResult(result("orphan_id"))  // Orphan result
    ]),
];

// Sanitize
let clean = sanitize(raw_messages, &snapshot, Role::Owner)?;

// RESULT:
// [
//     System: [Fresh snapshot with bootstrap, AGENTS.md, tree, git, tools],
//     User: [Text("[Time: 2026-08-17T14:32:19Z | Role: Owner]\nDebug this")],
//     Assistant: [ToolCall(id="synth_read_file_0", name="read_file", arguments={})],
// ]
// - System replaced
// - Metadata injected
// - Orphan result dropped
// - Tool call normalized (ID synthesized, name trimmed, arguments parsed)
```

---

## Error Handling

```mermaid
flowchart TD
    A[sanitize called] --> B{Messages<br/>empty?}
    B -->|Yes| C[SanitizerError::EmptyMessages]
    B -->|No| D[Stage 1: inject_system]
    D --> E[Stage 2: inject_metadata]
    E --> F[Stage 3: drop_orphans]
    F --> G[Stage 4: normalize_tool_calls]
    G -->|Error| H[SanitizerError from normalize]
    G -->|Ok| I[Stage 5: enforce_integrity]
    I --> J[Ok with clean messages]
    
    style C fill:#FF6B6B
    style H fill:#FF6B6B
    style J fill:#90EE90
```

### Error Types

| Error | Cause | Recovery |
|-------|-------|----------|
| `EmptyMessages` | Input array is empty | Fail early; SessionRunner prevents this |
| Normalize errors | Currently none; future validation may add | Return error to SessionRunner |

**Current Behavior**: All stages are **infallible** except empty input check. Invalid data is **fixed or dropped**, not rejected.

---

## Integration with SessionRunner

```mermaid
sequenceDiagram
    participant Runner as SessionRunner
    participant Sanitizer
    participant Snapshot as SnapshotBuilder
    participant Provider as NormalizeProvider
    
    Runner->>Runner: User sends message
    Runner->>Snapshot: snapshot()
    Snapshot-->>Runner: SessionSnapshot
    
    Runner->>Sanitizer: sanitize(messages, snapshot, role)
    
    Sanitizer->>Sanitizer: Stage 1: inject_system
    Sanitizer->>Sanitizer: Stage 2: inject_metadata
    Sanitizer->>Sanitizer: Stage 3: drop_orphans
    Sanitizer->>Sanitizer: Stage 4: normalize_tool_calls
    Sanitizer->>Sanitizer: Stage 5: enforce_integrity
    
    Sanitizer-->>Runner: Clean messages
    
    Runner->>Provider: convert_to_provider(clean_messages)
    Provider-->>Runner: Provider-specific format
    
    Runner->>Runner: Send to LLM API
```

**SessionRunner Call Sites**:
1. **Before every LLM request**: `run_turn()` → `sanitize()` → `convert_to_provider()`
2. **After compaction**: Compaction returns raw messages → `sanitize()` → continue
3. **After tool execution**: Tool results added → `sanitize()` → send next chunk

---

## Performance

| Stage | Complexity | Typical Time |
|-------|-----------|--------------|
| **Stage 1: System** | O(messages) | <1ms |
| **Stage 2: Metadata** | O(messages) | <1ms |
| **Stage 3a: Drop Orphan Results** | O(messages × content blocks) | <1ms |
| **Stage 3b: Drop Orphan Calls** | O(messages × content blocks) | 1-2ms |
| **Stage 4: Normalize** | O(messages × content blocks) | <1ms |
| **Stage 5a: Reorder** | O(messages × content blocks) | 1-3ms |
| **Stage 5b: Merge** | O(messages × content blocks) | <1ms |
| **Stage 5c: Deduplicate** | O(messages × content blocks) | 1-2ms |

**Total**: 5-10ms for typical 50-message array with 100 content blocks

---

## Properties Guaranteed After Sanitization

```mermaid
graph TD
    A[Clean Messages] --> B[System message at index 0]
    A --> C[Last user message has metadata]
    A --> D[All tool results have matching calls]
    A --> E[All tool calls have matching results]
    A --> F[Tool results appear after their calls]
    A --> G[Tool call IDs are unique]
    A --> H[Tool call names are trimmed]
    A --> I[Tool call IDs are non-empty]
    A --> J[Adjacent same-role messages merged]
    
    style A fill:#90EE90
```

**LLM Provider Compatibility**: Clean messages satisfy Anthropic, OpenAI, Google, and other provider requirements for message structure.

---

## Testing

```bash
# Run all tests
cargo test -p operon-context-sanitizer

# Run specific stage tests
cargo test -p operon-context-sanitizer --test system
cargo test -p operon-context-sanitizer --test orphans
cargo test -p operon-context-sanitizer --test integrity
```

**Test Coverage**:
- ✅ Each stage has isolated unit tests
- ✅ Pipeline integration tests with multi-issue messages
- ✅ Edge cases (empty arrays, missing fields, out-of-order blocks)

---

## License

Operon is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

---

> Built by **Luka Gray (aka Soumo Mukherjee)** • West Bengal, India • 2026
