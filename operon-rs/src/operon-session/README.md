# operon-session

**Agent loop orchestration, provider HTTP/SSE streaming, tool dispatch coordination, context compaction, and JSON-backed persistence**

`operon-session` is the **heart of Operon's agentic execution**. It sits between the frontend (TUI/GUI) and the context/tools pipeline, orchestrating the complete agent loop from user input to model response to tool execution.

---

## Overview

This crate provides `SessionRunner`, the central orchestrator that owns all session state and drives the full agentic cycle:

```mermaid
flowchart TB
    Frontend[Frontend TUI/GUI] -->|SessionConfig| Runner[SessionRunner::new]
    Runner -->|run user_message| Loop[Agent Loop]
    
    Loop --> Compaction[1. Check compaction threshold]
    Compaction --> Snapshot[2. Build snapshot]
    Snapshot --> Sanitize[3. Sanitize messages]
    Sanitize --> Tools[4. Collect tool definitions]
    Tools --> Request[5. Build provider request]
    Request --> Stream[6. Send + consume SSE stream]
    Stream --> Usage[7. Record token usage]
    Usage --> History[8. Push assistant message]
    History --> Persist[9. Persist turn to JSON]
    Persist --> Check{Tool calls?}
    Check -->|No| Done[Emit Done]
    Check -->|Yes| Policy[10. Policy check each call]
    Policy --> Dispatch[11. Dispatch tools]
    Dispatch --> Loop
    
    style Runner fill:#90EE90
    style Loop fill:#FFD700
    style Done fill:#87CEEB
```

**Key Features**:
- ✅ **Complete agent loop** — compaction → snapshot → stream → dispatch → repeat
- ✅ **HTTP/SSE streaming** — real-time token-by-token provider responses
- ✅ **Policy enforcement** — Allow/Ask/Deny gates before tool execution
- ✅ **JSON persistence** — human-readable session + turn storage
- ✅ **Lifecycle state machine** — Idle → Running → Done/Failed
- ✅ **Command channel** — Cancel/Approve/Deny from UI
- ✅ **11 provider support** — via operon-providers + operon-context normalize crates
- ✅ **Special tool handling** — ask-tool suspends loop for user input

---

## Architecture

### Crate Structure

```mermaid
graph TB
    A[operon-session] --> B[config.rs<br/>SessionConfig]
    A --> C[lifecycle.rs<br/>LifecycleState]
    A --> D[error.rs<br/>SessionError]
    A --> E[store.rs<br/>JSON persistence]
    A --> F[http.rs<br/>SSE streaming]
    A --> G[request.rs<br/>Request building]
    A --> H[runner/mod.rs<br/>SessionRunner]
    
    H --> I[runner/loop_impl.rs<br/>Agent loop]
    H --> J[runner/tool_dispatch.rs<br/>Tool handling]
    H --> K[runner/message_build.rs<br/>Message construction]
    H --> L[runner/compaction.rs<br/>Context compaction]
    H --> M[runner/commands.rs<br/>Command channel]
    H --> N[runner/policy_path.rs<br/>Path extraction]
    
    style A fill:#90EE90
    style H fill:#FFD700
```

---

### Dependency Graph

```mermaid
flowchart TB
    Session[operon-session] --> Config[operon-config<br/>PolicyConfig + paths]
    Session --> Context[operon-context<br/>Messages, Snapshot, Compaction]
    Session --> Events[operon-events<br/>SessionEvent, SessionCommand]
    Session --> Policy[operon-policy<br/>PolicyResolver]
    Session --> Providers[operon-providers<br/>Provider enum, ProviderConfig]
    Session --> Tools[operon-tools<br/>Dispatcher]
    Session --> ToolsAsk[operon-tools-ask<br/>AskArgs parsing]
    Session --> ToolsMemory[operon-tools-memory-store<br/>MemoryStore]
    
    Session --> Reqwest[reqwest<br/>HTTP client]
    Session --> Tokio[tokio<br/>Async runtime]
    Session --> Serde[serde, serde_json<br/>Serialization]
    
    style Session fill:#90EE90
```

---

## Core Types

### SessionRunner

**Purpose**: The agent loop orchestrator — owns all session state

```rust
pub struct SessionRunner {
    session_id: String,
    config: SessionConfig,
    messages: Vec<ConversationMessage>,
    dispatcher: Dispatcher,
    snapshot_builder: SnapshotBuilder,
    token_state: SessionTokenState,
    token_budget: TokenBudget,
    lifecycle: LifecycleState,
    http_client: Client,
    event_tx: mpsc::Sender<SessionEvent>,
    cmd_rx: mpsc::Receiver<SessionCommand>,
    policy_resolver: PolicyResolver,
    pending_commands: VecDeque<SessionCommand>,
    store: Option<SessionStore>,
    turn_index: usize,
}
```

---

### SessionConfig

**Purpose**: All runtime parameters for a SessionRunner

```mermaid
classDiagram
    class SessionConfig {
        +provider_config: ProviderConfig
        +policy: PolicyConfig
        +project_dir: Option~PathBuf~
        +workspace_root: PathBuf
        +role: Role
        +tool_groups: Vec~String~
        +compaction: CompactionConfig
        +store_path: Option~PathBuf~
        +channel_instructions: Option~String~
        +default_tool_groups() Vec~String~
        +snapshot_config(session_id) SnapshotConfig
    }
    
    SessionConfig --> ProviderConfig
    SessionConfig --> PolicyConfig
    SessionConfig --> CompactionConfig
```

---

#### Directory Model (3 Directions)

```mermaid
flowchart TB
    D1[Direction 1: Default Workspace<br/>~/.operon/workspace/<br/>Always accessible]
    D2[Direction 2: Allowed Directories<br/>Listed in config.toml<br/>Configurable permissions]
    D3[Direction 3: Project Directory<br/>VS Code-style open<br/>Becomes snapshot root]
    
    Mode{Session Mode}
    Mode -->|project_dir = None| Normal[NORMAL MODE<br/>workspace_root = ~/.operon/workspace/]
    Mode -->|project_dir = Some| Project[PROJECT MODE<br/>workspace_root = project_dir]
    
    Normal --> Snap1[Snapshot reads from<br/>~/.operon/workspace/]
    Project --> Snap2[Snapshot reads from<br/>project_dir]
    
    style D1 fill:#90EE90
    style D2 fill:#87CEEB
    style D3 fill:#FFD700
```

**Direction 1 (Default Workspace)**:
- Path: `~/.operon/workspace/`
- Always accessible to the agent
- Injected into policy by operon-config
- Snapshot root in NORMAL mode

**Direction 2 (Allowed Directories)**:
- Loaded from `config.toml` `[[directories]]` blocks
- Configurable per-directory permissions (filesystem + shell)
- Checked by PolicyResolver at runtime

**Direction 3 (Project Directory)**:
- Opened VS Code-style via `project_dir: Some(path)`
- Becomes workspace_root (snapshot root) in PROJECT mode
- Must already exist in config.toml as a Direction 2 directory
- No runtime policy changes — permissions configured in advance

---

### LifecycleState

**Purpose**: State machine for session execution

```mermaid
stateDiagram-v2
    [*] --> Idle: new()
    Idle --> Running: run()
    Running --> Paused: pause()
    Paused --> Running: resume()
    Running --> Done: No tool calls
    Running --> Failed: Fatal error
    Done --> [*]
    Failed --> [*]
    
    note right of Running
        Agent loop active
        HTTP/tool dispatch in-flight
    end note
    
    note right of Paused
        Awaiting user approval
        or ask-tool response
    end note
    
    note right of Done
        Natural completion
        EndTurn with no tools
    end note
    
    note right of Failed
        Unrecoverable error
        HTTP failure, parse error
    end note
```

```rust
pub enum LifecycleState {
    Idle,     // Created but not started
    Running,  // Loop actively executing
    Paused,   // Waiting for approval/input
    Done,     // Natural completion
    Failed,   // Fatal error occurred
}
```

**State Guards**:
```rust
impl LifecycleState {
    pub fn can_run(&self) -> bool {
        matches!(self, Self::Idle | Self::Paused)
    }
    
    pub fn can_pause(&self) -> bool {
        matches!(self, Self::Running)
    }
}
```

---

### SessionError

**Purpose**: Unified error type for all session operations

```mermaid
classDiagram
    class SessionError {
        <<enumeration>>
        Http(reqwest Error)
        Stream(String)
        Normalize(String)
        Sanitizer(SanitizerError)
        Snapshot(SnapshotError)
        Compaction(CompactionError)
        Store(String)
        InvalidState{state String}
        Config(String)
        Memory(MemoryStoreError)
    }
    
    SessionError --> reqwest_Error
    SessionError --> SanitizerError
    SessionError --> SnapshotError
    SessionError --> CompactionError
    SessionError --> MemoryStoreError
```

---

## Agent Loop Implementation

### Per-Turn Cycle (run method)

**Module**: `runner/loop_impl.rs`

```mermaid
sequenceDiagram
    participant UI as Frontend
    participant Runner as SessionRunner
    participant Snap as SnapshotBuilder
    participant HTTP as send_streaming
    participant Disp as Dispatcher
    participant Store as SessionStore
    
    UI->>Runner: run(user_message)
    Runner->>Runner: Check lifecycle.can_run()
    Runner->>Runner: Push user message to history
    
    loop Agent Loop
        Runner->>Runner: Should compact?
        alt Token budget exceeded
            Runner->>Runner: run_compaction()
            Runner->>UI: CompactionOccurred
        end
        
        Runner->>Snap: build()
        Snap-->>Runner: Snapshot
        Runner->>Runner: sanitize(messages, snapshot)
        Runner->>Disp: definitions()
        Disp-->>Runner: Vec<ToolDefinition>
        Runner->>Runner: build_request()
        Runner->>UI: PreTurnReady
        
        Runner->>HTTP: send_streaming()
        HTTP->>UI: TextDelta events
        HTTP->>UI: ThinkingDelta events
        HTTP->>UI: ToolCallStart events
        HTTP-->>Runner: StreamResult
        
        Runner->>Runner: Record token usage
        Runner->>UI: TokenUsageUpdated
        Runner->>UI: ContextUsageUpdated
        Runner->>Runner: Push assistant message
        Runner->>Store: save_turn()
        
        alt No tool calls
            Runner->>UI: TurnComplete
            Runner->>UI: Done
            Runner->>Runner: lifecycle = Done
            Note over Runner: Break loop
        else Has tool calls
            Runner->>Runner: Check for Cancel
            loop Each tool call
                Runner->>Runner: handle_tool_call()
                alt ask tool
                    Runner->>UI: AskQuestion
                    UI->>Runner: AskResponse
                    Runner->>Runner: Return answer
                else Policy: Ask
                    Runner->>UI: ApprovalRequired
                    UI->>Runner: Approve/Deny
                    alt Approved
                        Runner->>Disp: dispatch()
                        Disp-->>Runner: ToolResult
                    else Denied
                        Runner->>UI: PermissionDenied
                    end
                else Policy: Allow
                    Runner->>Disp: dispatch()
                    Disp-->>Runner: ToolResult
                else Policy: Deny
                    Runner->>UI: PermissionDenied
                end
                Runner->>UI: ToolCallResult
            end
            Runner->>Runner: Push tool results
            Note over Runner: Loop back to compaction check
        end
    end
    
    Runner-->>UI: Done/Failed
```

---

### Compaction Flow

**Module**: `runner/compaction.rs`

```mermaid
flowchart TB
    Start[Token budget exceeded] --> Check{Provider?}
    Check -->|Anthropic| Client[Build AnthropicCompactionClient]
    Check -->|Other| Warn[Emit Warning<br/>Skip compaction]
    
    Client --> Compact[Call compact pipeline]
    Compact --> Rebuild[Rebuild message history]
    Rebuild --> Reset[Reset token_state]
    Reset --> Notify[dispatcher.notify_compaction]
    Notify --> Event[Emit CompactionOccurred]
    Event --> Update[Emit ContextUsageUpdated]
    Update --> Done[Return Ok]
    
    Warn --> Done
    
    style Start fill:#FFD700
    style Compact fill:#90EE90
    style Done fill:#87CEEB
```

**Supported Providers**:
- ✅ **Anthropic** — Full compaction support via AnthropicCompactionClient
- ❌ **Others** — Logs warning + emits Warning event, skips compaction

---

### Tool Dispatch Flow

**Module**: `runner/tool_dispatch.rs`

```mermaid
flowchart TB
    Start[handle_tool_call] --> Ask{Tool == ask?}
    
    Ask -->|Yes| ParseArgs[Parse AskArgs]
    ParseArgs -->|Error| ReturnErr[Return error ToolResult]
    ParseArgs -->|Ok| EmitQ[Emit AskQuestion]
    EmitQ --> WaitAns[Wait for AskResponse]
    WaitAns -->|Cancel| StopFlow[Return Stop]
    WaitAns -->|Answer| ReturnAns[Return answer ToolResult]
    
    Ask -->|No| PolicyCheck{Policy check}
    
    PolicyCheck -->|Allow| Dispatch[dispatcher.dispatch_with_progress]
    
    PolicyCheck -->|Ask| EmitApp[Emit ApprovalRequired]
    EmitApp --> WaitApp[Wait for Approve/Deny/Cancel]
    WaitApp -->|Approve| Granted[Emit ApprovalGranted]
    Granted --> Dispatch
    WaitApp -->|Deny| Denied[Emit PermissionDenied]
    Denied --> OpaqueErr[Return opaque error]
    WaitApp -->|Cancel| StopFlow
    
    PolicyCheck -->|Deny| Denied2[Emit PermissionDenied]
    Denied2 --> OpaqueErr
    
    Dispatch --> EmitRes[Emit ToolCallResult]
    EmitRes --> PushRes[Push to tool_results]
    PushRes --> Continue[Return Continue]
    
    ReturnErr --> Continue
    ReturnAns --> Continue
    OpaqueErr --> Continue
    
    style Start fill:#FFD700
    style Dispatch fill:#90EE90
    style Continue fill:#87CEEB
    style StopFlow fill:#FF6B6B
```

**Return Values**:
```rust
pub enum ToolCallFlow {
    Continue,  // Process next tool call
    Stop,      // Break out of loop (Cancel received)
}
```

---

## HTTP Streaming

**Module**: `http.rs`

### SSE Stream Consumption

```mermaid
sequenceDiagram
    participant Runner as SessionRunner
    participant HTTP as send_streaming
    participant Stream as bytes_stream
    participant Parser as parse_line
    participant Asm as Assembler
    participant UI as event_tx
    
    Runner->>HTTP: send_streaming(body, event_tx, cmd_rx)
    HTTP->>HTTP: build_headers(provider, api_key)
    HTTP->>HTTP: POST request
    HTTP->>HTTP: Check status.is_success()
    HTTP->>Stream: bytes_stream()
    
    loop Consume chunks
        Stream->>HTTP: chunk
        HTTP->>HTTP: Accumulate into line_buf
        
        alt Newline found
            HTTP->>HTTP: Extract "data: " payload
            HTTP->>Parser: parse_line(payload, provider)
            Parser-->>HTTP: Vec<StreamEvent>
            
            loop Each event
                HTTP->>Asm: push(event)
                Asm-->>HTTP: AssemblerOutput
                
                alt Text delta
                    HTTP->>UI: TextDelta
                    HTTP->>HTTP: Accumulate in result.text
                else Reasoning delta
                    HTTP->>UI: ThinkingDelta
                else Reasoning block
                    HTTP->>HTTP: Store in result.reasoning
                else ToolCall complete
                    HTTP->>UI: ToolCallStart
                    HTTP->>UI: ToolCallArgsReady
                    HTTP->>HTTP: Push to result.tool_calls
                else StreamEnded
                    HTTP->>HTTP: Store stop_reason
                else Pending
                    Note over HTTP: Assembler buffering
                end
            end
        end
        
        alt Cancel received on cmd_rx
            HTTP->>HTTP: Break loop
        end
    end
    
    HTTP->>Asm: finish()
    Asm-->>HTTP: Final outputs
    HTTP-->>Runner: StreamResult
```

---

### StreamResult

```rust
pub struct StreamResult {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop_reason: Option<StopReason>,
    pub usage_raw: Option<Value>,
    pub reasoning: Option<ReasoningBlock>,
}
```

---

### Header Construction

```mermaid
flowchart TB
    Start[build_headers] --> Type[Set Content-Type: application/json]
    Type --> Check{Provider?}
    
    Check -->|Anthropic| Anthro[Set x-api-key: api_key]
    Anthro --> Version[Set anthropic-version: 2023-06-01]
    
    Check -->|Others| Bearer[Set Authorization: Bearer api_key]
    
    Version --> Return[Return HeaderMap]
    Bearer --> Return
    
    style Start fill:#FFD700
    style Return fill:#87CEEB
```

**Provider-Specific Headers**:

| Provider | Auth Header | Extra Headers |
|----------|-------------|---------------|
| **Anthropic** | `x-api-key: {key}` | `anthropic-version: 2023-06-01` |
| **All Others** | `Authorization: Bearer {key}` | None |

---

## Request Building

**Module**: `request.rs`

### Build Flow

```mermaid
flowchart TB
    Start[build_request] --> Denorm[denormalize_messages<br/>canonical → wire]
    Denorm --> Extract[Extract messages array<br/>+ system value]
    Extract --> Tools[denormalize_definition<br/>for each ToolDefinition]
    Tools --> Assemble{Provider?}
    
    Assemble -->|Anthropic| AnthBody[Build body with<br/>top-level system field]
    AnthBody --> AnthTools{tools empty?}
    AnthTools -->|No| AddTools[Add tools array]
    AnthTools -->|Yes| Skip1[Skip tools field]
    AddTools --> Return[Return JSON body]
    Skip1 --> Return
    
    Assemble -->|Others| OtherBody[Build body with<br/>system in messages array]
    OtherBody --> OtherTools{tools empty?}
    OtherTools -->|No| AddTools2[Add tools array]
    OtherTools -->|Yes| Skip2[Skip tools field]
    AddTools2 --> Return
    Skip2 --> Return
    
    style Start fill:#FFD700
    style Return fill:#87CEEB
```

---

### Provider Endpoints

```rust
pub fn provider_endpoint(provider: &Provider) -> &'static str {
    match provider {
        Provider::Anthropic   => "https://api.anthropic.com/v1/messages",
        Provider::OpenAI      => "https://api.openai.com/v1/chat/completions",
        Provider::DeepSeek    => "https://api.deepseek.com/v1/chat/completions",
        Provider::OpenRouter  => "https://openrouter.ai/api/v1/chat/completions",
        Provider::Groq        => "https://api.groq.com/openai/v1/chat/completions",
        Provider::Mistral     => "https://api.mistral.ai/v1/chat/completions",
        Provider::XAI         => "https://api.x.ai/v1/chat/completions",
        Provider::NvidiaNim   => "https://integrate.api.nvidia.com/v1/chat/completions",
        Provider::Ollama      => "http://localhost:11434/v1/chat/completions",
        Provider::Gemini      => "https://generativelanguage.googleapis.com/v1beta/models",
        Provider::Cohere      => "https://api.cohere.com/v2/chat",
    }
}
```

**Note**: Runtime uses `ProviderConfig::effective_base_url()` instead to respect `base_url_override`

---

## JSON Persistence

**Module**: `store.rs`

### Design (Moved from SQLite to JSON)

```mermaid
flowchart LR
    Old[❌ OLD: SQLite<br/>Binary, opaque<br/>Connection issues] --> New[✅ NEW: JSON<br/>Human-readable<br/>Easy debugging]
    
    New --> File[One file per session<br/>~/.operon/sessions/ID.json]
    File --> Atomic[Atomic read/write<br/>serde_json + fs]
    
    style Old fill:#FF6B6B
    style New fill:#90EE90
```

**Why JSON over SQLite**:
1. **Human-readable** — Open in VS Code, inspect directly
2. **No lock issues** — Simple file operations, no connection pooling
3. **Simpler debugging** — cat/grep the JSON file
4. **Transparent** — See exact conversation structure

---

### SessionStore

```rust
pub struct SessionStore {
    path: PathBuf,  // ~/.operon/sessions/<session_id>.json
}
```

**Methods**:
```rust
impl SessionStore {
    pub async fn open(path: &Path) -> Result<Self, SessionError>;
    pub async fn create_session(id, workspace, model_id, provider) -> Result<(), SessionError>;
    pub async fn save_turn(session_id, turn_index, messages, token_count) -> Result<(), SessionError>;
    pub async fn truncate_turns(session_id, keep_turns_count) -> Result<(), SessionError>;
    pub async fn load_turns(session_id) -> Result<Vec<Vec<ConversationMessage>>, SessionError>;
    pub async fn load_turns_with_timestamps(session_id) -> Result<Vec<(i64, Vec<ConversationMessage>)>, SessionError>;
    pub async fn load_full_history(session_id) -> Result<Vec<ConversationMessage>, SessionError>;
    pub async fn list_sessions() -> Result<Vec<SessionRow>, SessionError>;
    pub async fn get_last_token_count(session_id) -> Result<Option<usize>, SessionError>;
    pub async fn get_first_user_message_text(session_id) -> Result<Option<String>, SessionError>;
}
```

---

### JSON Schema

```json
{
  "id": "session-abc123",
  "created_at": 1735689600,
  "workspace": "/home/user/project",
  "model_id": "claude-sonnet-4-20250514",
  "provider": "Anthropic",
  "turns": [
    {
      "turn_index": 0,
      "messages": [
        {
          "role": "user",
          "content": [{"Text": "Hello!"}],
          "stop_reason": null
        },
        {
          "role": "assistant",
          "content": [{"Text": "Hi there!"}],
          "stop_reason": {"EndTurn": {}}
        }
      ],
      "token_count": 1024,
      "created_at": 1735689610
    }
  ]
}
```

---

### Persistence Flow

```mermaid
sequenceDiagram
    participant Runner as SessionRunner
    participant Store as SessionStore
    participant FS as Filesystem
    
    Runner->>Store: new() with store_path
    Store->>FS: create_dir_all(parent)
    Store-->>Runner: SessionStore
    
    Runner->>Store: create_session(id, workspace, model, provider)
    Store->>Store: Build SessionJson with empty turns
    Store->>FS: write JSON (pretty format)
    
    loop Each turn
        Runner->>Runner: Execute turn (HTTP + tools)
        Runner->>Store: save_turn(turn_index, messages, token_count)
        Store->>FS: read current JSON
        Store->>Store: Parse SessionJson
        Store->>Store: Update/append turn
        Store->>Store: Sort by turn_index
        Store->>FS: write JSON (pretty format)
    end
    
    Runner->>Store: load_full_history()
    Store->>FS: read JSON
    Store->>Store: Parse + flatten turns
    Store-->>Runner: Vec<ConversationMessage>
```

---

## Command Channel

**Module**: `runner/commands.rs`

### Command Flow

```mermaid
flowchart TB
    UI[Frontend sends<br/>SessionCommand] --> Chan[mpsc channel<br/>cmd_rx]
    Chan --> Drain[drain_ready_commands<br/>→ pending_commands]
    Drain --> Buffer[VecDeque buffer]
    
    Buffer --> Wait[wait_for_relevant_command<br/>approval_id]
    Wait --> Match{command_matches?}
    Match -->|Yes| Return[Return command]
    Match -->|No| Rebuf[Push to buffer]
    Rebuf --> Wait
    
    style UI fill:#FFD700
    style Buffer fill:#87CEEB
    style Return fill:#90EE90
```

---

### SessionCommand

```rust
pub enum SessionCommand {
    Cancel,
    Approve { id: String },
    Deny { id: String },
    AskResponse { id: String, answer: String },
}
```

---

### Matching Logic

```rust
fn command_matches(command: &SessionCommand, approval_id: Option<&str>) -> bool {
    match command {
        SessionCommand::Cancel => true,  // Always matches
        SessionCommand::Approve { id }
        | SessionCommand::Deny { id }
        | SessionCommand::AskResponse { id, .. } => {
            approval_id.is_some_and(|expected| expected == id)
        }
    }
}
```

**Behavior**:
- **Cancel** — Always matches (global stop signal)
- **Approve/Deny/AskResponse** — Match only if ID matches expected approval_id
- Irrelevant commands are buffered in `pending_commands` for later processing

---

## Special Tool Handling

### ask Tool Interception

**Module**: `runner/tool_dispatch.rs`

```mermaid
sequenceDiagram
    participant Runner as SessionRunner
    participant UI as Frontend
    
    Runner->>Runner: Detect call.name == "ask"
    Runner->>Runner: Parse AskArgs from arguments
    
    alt Parse error
        Runner->>UI: ToolCallResult (error)
        Runner->>Runner: Continue to next call
    else Parse success
        Runner->>UI: AskQuestion(id, question, options)
        Runner->>Runner: wait_for_relevant_command(id)
        
        alt Receive AskResponse
            UI->>Runner: AskResponse { id, answer }
            Runner->>UI: ToolCallResult (success, answer)
            Runner->>Runner: Continue to next call
        else Receive Cancel
            UI->>Runner: Cancel
            Runner->>Runner: Return Stop (break loop)
        end
    end
```

**Key Difference**: ask tool **bypasses Dispatcher** — no tool implementation needed, handled entirely in runner

---

### Upfront Tool Exposure

All registered tools (`fs`, `shell`, `web`, `todo`, `ask`, `memory`) are exposed to the model directly in the top-level API payload from turn 1. This eliminates discovery round-trip latency, supports provider prompt caching, and avoids polluting conversation context with dynamic schema JSON.

---

## Usage Examples

### Basic Session Creation

```rust
use operon_session::{SessionConfig, SessionRunner};
use operon_config::load;
use operon_events::{SessionEvent, SessionCommand};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load application config
    let app = load()?;
    
    // Build session config
    let config = SessionConfig {
        provider_config: app.provider,
        policy: app.policy,
        project_dir: None,
        workspace_root: app.paths.workspace_dir.clone(),
        role: operon_context::Role::Owner,
        tool_groups: SessionConfig::default_tool_groups(),
        compaction: operon_context::CompactionConfig::default(),
        store_path: Some(app.paths.session_db("my-session")),
        channel_instructions: None,
    };
    
    // Create event/command channels
    let (event_tx, mut event_rx) = mpsc::channel(100);
    let (cmd_tx, cmd_rx) = mpsc::channel(10);
    
    // Create runner
    let mut runner = SessionRunner::new(config, event_tx, cmd_rx).await?;
    
    // Spawn event listener
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                SessionEvent::TextDelta { text } => print!("{}", text),
                SessionEvent::Done => println!("\n[Session complete]"),
                _ => {}
            }
        }
    });
    
    // Run agent loop
    runner.run("Hello!".to_string(), vec![], vec![]).await?;
    
    Ok(())
}
```

---

### Project Mode Session

```rust
use std::path::PathBuf;

let project_path = PathBuf::from("/home/user/my-project");

let config = SessionConfig {
    project_dir: Some(project_path.clone()),
    workspace_root: project_path,  // Snapshot root = project dir
    // ... other fields
};

// AGENTS.md, tree, git status all read from /home/user/my-project
```

---

### Sending Cancel Command

```rust
use operon_events::SessionCommand;

// From UI thread
cmd_tx.send(SessionCommand::Cancel).await?;

// Runner will:
// 1. Break stream consumption if mid-stream
// 2. Break tool-call loop if dispatching
// 3. Emit Done event
// 4. Transition to Done state
```

---

### Handling Approval Required

```rust
// Runner emits:
SessionEvent::ApprovalRequired {
    id: "abc123",
    tool: "bash",
    path: Some("/etc/passwd"),
    reason: "Filesystem operation in restricted directory",
    args_json: "...".to_string(),
}

// UI displays approval dialog

// User approves:
cmd_tx.send(SessionCommand::Approve { id: "abc123".to_string() }).await?;

// OR denies:
cmd_tx.send(SessionCommand::Deny { id: "abc123".to_string() }).await?;
```

---

### ask Tool Flow

```rust
// Runner emits:
SessionEvent::AskQuestion {
    id: "q1",
    question: "Which approach should I use?",
    options: vec!["Option A".into(), "Option B".into()],
}

// UI renders multiple-choice widget

// User answers:
cmd_tx.send(SessionCommand::AskResponse {
    id: "q1".to_string(),
    answer: "Option A".to_string(),
}).await?;

// Runner receives answer and continues loop
```

---

### Resuming a Session

```rust
use operon_session::SessionStore;

let store = SessionStore::open(&app.paths.session_db("session-abc")).await?;

// Load history
let turns = store.load_turns("session-abc").await?;
let full_history = turns.into_iter().flatten().collect();
let last_token_count = store.get_last_token_count("session-abc").await?;

// Restore runner state
runner.set_history(full_history, 3, last_token_count);

// Continue with new user message
runner.run("Continue where we left off".to_string(), vec![], vec![]).await?;
```

---

## Error Handling

### Error Propagation

```mermaid
flowchart TB
    Start[Operation fails] --> Wrap{Error type?}
    
    Wrap -->|reqwest Error| Http[SessionError::Http]
    Wrap -->|SSE parse| Stream[SessionError::Stream]
    Wrap -->|Sanitizer| San[SessionError::Sanitizer]
    Wrap -->|Snapshot| Snap[SessionError::Snapshot]
    Wrap -->|Compaction| Comp[SessionError::Compaction]
    Wrap -->|Store| Store[SessionError::Store]
    Wrap -->|InvalidState| State[SessionError::InvalidState]
    
    Http --> Event[Emit SessionEvent::Error]
    Stream --> Event
    San --> Event
    Snap --> Event
    Comp --> Event
    Store --> Event
    State --> Event
    
    Event --> Lifecycle[lifecycle = Failed]
    Lifecycle --> Return[Return Err]
    
    style Start fill:#FF6B6B
    style Return fill:#FF6B6B
```

---

### PreTurnFailed Event

Emitted when errors occur BEFORE the HTTP request is sent:

```rust
SessionEvent::PreTurnFailed {
    turn_index: usize,
    step: PreTurnStep,  // Compaction | Snapshot | Sanitizer
    reason: String,
}
```

**Triggered by**:
- Compaction fatal errors (not threshold/insufficient history)
- Snapshot build failures (filesystem errors, watcher issues)
- Sanitizer validation failures

---

### Compaction Error Handling

```mermaid
flowchart TB
    Start[run_compaction] --> Call[Call compact pipeline]
    Call --> Result{Result?}
    
    Result -->|Ok| Update[Update messages + token_state]
    Update --> Emit[Emit CompactionOccurred]
    Emit --> Return[Return Ok]
    
    Result -->|Err: ThresholdNotReached| Warn1[Log warning + Return Ok]
    Result -->|Err: InsufficientHistory| Warn2[Emit Warning event + Return Ok]
    Result -->|Err: Other| Fatal[Emit PreTurnFailed]
    
    Fatal --> Fail[lifecycle = Failed]
    Fail --> ReturnErr[Return Err]
    
    style Update fill:#90EE90
    style Warn1 fill:#FFD700
    style Warn2 fill:#FFD700
    style Fatal fill:#FF6B6B
```

---

## Testing

```bash
# Run all tests
cargo test -p operon-session

# Run with logs
cargo test -p operon-session -- --nocapture

# Test store persistence
cargo test -p operon-session store::tests

# Test lifecycle state machine
cargo test -p operon-session lifecycle::tests
```

---

## Dependencies

```toml
[dependencies]
operon-config                = { workspace = true }
operon-context               = { workspace = true, features = ["http-client"] }
operon-events                = { workspace = true }
operon-policy                = { workspace = true }
operon-providers             = { workspace = true }
operon-tools                 = { workspace = true }
operon-tools-ask             = { workspace = true }
operon-tools-memory-store    = { workspace = true }

tokio      = { workspace = true }
reqwest    = { workspace = true, features = ["json", "stream"] }
futures    = { workspace = true }
serde      = { workspace = true }
serde_json = { workspace = true }
tracing    = { workspace = true }
thiserror  = { workspace = true }
async-trait = { workspace = true }
sqlx = { workspace = true }  # Legacy, not actively used post-JSON migration
```

---

## Design Rationale

### Why JSON Over SQLite?

```mermaid
graph TD
    A[❌ SQLite Issues] --> B[Binary format<br/>opaque]
    A --> C[Connection pooling<br/>lock contention]
    A --> D[Schema migrations]
    A --> E[Harder debugging]
    
    F[✅ JSON Benefits] --> G[Human-readable<br/>cat/grep works]
    F --> H[Simple file ops<br/>no connections]
    F --> I[No schema<br/>version issues]
    F --> J[Easy debugging<br/>VS Code preview]
    
    style A fill:#FF6B6B
    style F fill:#90EE90
```

---

### Why Split runner/ into Submodules?

**Before** (single `runner.rs`):
- 2000+ LOC monolithic file
- Difficult to navigate
- Cognitive overload

**After** (7 submodules):
- Each <400 LOC
- Clear separation of concerns
- Easy to locate functionality

```
runner/
  mod.rs           — SessionRunner struct + new()
  loop_impl.rs     — Per-turn agent loop
  tool_dispatch.rs — Per-tool-call handling
  message_build.rs — Pure message construction
  compaction.rs    — Compaction orchestration
  commands.rs      — Command channel plumbing
  policy_path.rs   — Path extraction for policy
```

---

### Why ask Tool Special Case?

```mermaid
flowchart LR
    Normal[Normal Tool] --> Disp[Dispatcher]
    Disp --> Impl[Tool Implementation]
    Impl --> Result[ToolResult]
    
    Ask[ask Tool] --> Intercept[Intercepted in runner]
    Intercept --> UI[UI renders question]
    UI --> User[User answers]
    User --> Direct[Direct ToolResult]
    
    style Normal fill:#87CEEB
    style Ask fill:#FFD700
```

**Why**:
- ask tool MUST suspend the entire loop
- Dispatcher is synchronous — cannot await UI response
- Direct handling in runner allows `.await` on command channel

---

## License

Operon is licensed under the **GNU Affero General Public License v3.0 (AGPLv3)**.

---

> Built by **Luka Gray (aka Soumo Mukherjee)** • West Bengal, India • 2026
