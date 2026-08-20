<div align="center">

<img src="../assets/logo.svg" width="72" alt="Operon" />

# Operon — VS Code Extension

*The full Operon agent, living inside your editor.*

[![VS Code](https://img.shields.io/badge/VS%20Code-%3E%3D1.90-blue?style=flat-square&logo=visualstudiocode)](https://code.visualstudio.com/)
[![Rust](https://img.shields.io/badge/Rust-2021-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.4-blue?style=flat-square&logo=typescript)](https://www.typescriptlang.org/)
[![License](https://img.shields.io/badge/License-AGPL--3.0-green?style=flat-square)](../../LICENSE)

</div>

---

## Overview

The Operon VS Code extension brings the full `operon-rs` agent runtime directly into your editor. It is **not** a thin API wrapper — it runs the same production Rust agent that powers the GUI and TUI, embedded as a native sidecar process.

> **No cloud required.** The entire agent loop runs locally on your machine.

---

## Architecture

The extension is split into two layers that communicate over **stdin/stdout JSON-RPC**:

```mermaid
graph TD
    subgraph "VS Code Process (Node.js)"
        EXT["extension.ts<br/>activate() / deactivate()"]
        BRIDGE_CLIENT["bridge.ts<br/>BridgeClient"]
        PANEL["panel.ts<br/>ChatViewProvider"]
        WEBVIEW["WebviewView<br/>media/chat.js + chat.css"]
        RPC_TS["rpc.ts<br/>Protocol Types"]
    end

    subgraph "Sidecar Process (Native Rust)"
        MAIN["main.rs<br/>stdin event loop"]
        HANDLER["handler.rs<br/>Request Dispatcher"]
        RPC_RS["rpc.rs<br/>Protocol Types"]
        OPERON["operon-rs<br/>Agent Runtime"]
    end

    subgraph "operon-rs Subsystems"
        SESSION["operon-session<br/>Agent Loop"]
        TOOLS["operon-tools<br/>fs, shell, web, memory"]
        POLICY["operon-policy<br/>Permission Gating"]
        CONFIG["operon-config<br/>config.toml"]
        STORE["SQLite<br/>~/.operon/sessions/"]
    end

    EXT --> BRIDGE_CLIENT
    EXT --> PANEL
    PANEL --> BRIDGE_CLIENT
    PANEL --> WEBVIEW
    BRIDGE_CLIENT -.->|"stdin: JSON-RPC request"| MAIN
    MAIN -.->|"stdout: JSON-RPC event"| BRIDGE_CLIENT
    BRIDGE_CLIENT --- RPC_TS

    MAIN --> HANDLER
    HANDLER --- RPC_RS
    HANDLER --> OPERON
    OPERON --> SESSION
    SESSION --> TOOLS
    SESSION --> POLICY
    SESSION --> CONFIG
    SESSION --> STORE

    style EXT fill:#0066B8,stroke:#004C8A,color:#fff
    style BRIDGE_CLIENT fill:#0066B8,stroke:#004C8A,color:#fff
    style PANEL fill:#0066B8,stroke:#004C8A,color:#fff
    style WEBVIEW fill:#0066B8,stroke:#004C8A,color:#fff
    style RPC_TS fill:#0066B8,stroke:#004C8A,color:#fff
    style MAIN fill:#D97706,stroke:#B45309,color:#fff
    style HANDLER fill:#D97706,stroke:#B45309,color:#fff
    style RPC_RS fill:#D97706,stroke:#B45309,color:#fff
    style OPERON fill:#16A34A,stroke:#15803D,color:#fff
    style SESSION fill:#7C3AED,stroke:#6D28D9,color:#fff
    style TOOLS fill:#7C3AED,stroke:#6D28D9,color:#fff
    style POLICY fill:#7C3AED,stroke:#6D28D9,color:#fff
    style CONFIG fill:#7C3AED,stroke:#6D28D9,color:#fff
    style STORE fill:#374151,stroke:#1F2937,color:#fff
```

---

## The Sidecar Pattern

VS Code extensions run in a **Node.js** process. Rust code cannot be linked directly into Node.js. The bridge sidecar solves this cleanly — the extension spawns `operon-vscode-bridge` as a child process on activation and communicates with it over its stdio pipes.

```mermaid
sequenceDiagram
    actor User
    participant WV as Webview (chat.js)
    participant EXT as Extension Host (TS)
    participant BR as Bridge (Rust)
    participant AG as operon-rs

    User->>WV: Types a prompt, hits Enter
    WV->>EXT: postMessage { type: "submit_prompt" }
    EXT->>BR: stdin → { id:1, method:"submit_prompt", params:{...} }

    loop Streaming agent loop
        BR->>AG: SessionRunner::run(prompt)
        AG-->>BR: SessionEvent::TextDelta
        BR-->>EXT: stdout → { id:1, event:"text_delta", data:{...} }
        EXT-->>WV: postMessage { type:"text_delta" }
        WV-->>User: Renders streamed text
    end

    AG-->>BR: SessionEvent::ToolUse (needs approval)
    BR-->>EXT: stdout → { id:1, event:"permission_req", data:{...} }
    EXT-->>User: VS Code notification "Approve / Deny"
    User->>EXT: Clicks "Approve"
    EXT->>BR: stdin → { id:2, method:"approve_permission" }
    BR->>AG: SessionCommand::Approve

    AG-->>BR: agent loop completes
    BR-->>EXT: stdout → { id:1, event:"agent_finished" }
    EXT-->>WV: postMessage { type:"agent_finished" }
```

---

## JSON-RPC Protocol

All messages are **newline-delimited JSON** on stdin/stdout. Stderr is reserved for diagnostic logs.

### Requests  *(Extension → Bridge)*

| Method | Params | Description |
|---|---|---|
| `submit_prompt` | `session_id?`, `prompt`, `workspace_path?` | Start or continue an agent session |
| `cancel` | — | Cancel the currently running prompt |
| `approve_permission` | `permission_id` | Approve a pending tool permission |
| `deny_permission` | `permission_id` | Deny a pending tool permission |
| `load_history` | `session_id` | Load message history for a session |

### Events  *(Bridge → Extension)*

| Event | Data | Description |
|---|---|---|
| `text_delta` | `{ delta }` | Streamed LLM text chunk |
| `tool_start` | `{ tool, label, input }` | A tool call has started |
| `tool_result` | `{ tool, success, summary }` | A tool call completed |
| `tool_progress` | `{ tool, stage, message }` | Mid-tool progress update |
| `permission_req` | `{ permission_id, tool, description }` | User approval required |
| `token_update` | `{ used, budget }` | Context window token state |
| `agent_finished` | `{ session_id }` | Agent loop completed |
| `agent_error` | `{ message }` | Agent loop failed |

---

## Directory Layout (Example)

```
vscode/
│
├── extension/                    # TypeScript — the .vsix package
│   ├── src/
│   │   ├── extension.ts          # activate() / deactivate()
│   │   ├── bridge.ts             # BridgeClient — child process + stdio RPC
│   │   ├── rpc.ts                # Protocol types (mirrors bridge/src/rpc.rs)
│   │   └── panel.ts              # ChatViewProvider — sidebar WebviewView
│   ├── media/                    # Chat UI assets bundled into .vsix
│   │   ├── chat.js               # Webview JavaScript (to be built)
│   │   └── chat.css              # Webview styles (to be built)
│   ├── assets/
│   │   ├── icon.png              # Marketplace icon (128x128)
│   │   └── icon-mono.svg         # Activity bar icon (monochrome)
│   ├── bin/                      # Pre-built bridge binaries (CI artefacts)
│   │   ├── operon-vscode-bridge.exe   # Windows
│   │   ├── operon-vscode-bridge-linux # Linux
│   │   └── operon-vscode-bridge-mac   # macOS
│   ├── dist/                     # esbuild output (gitignored)
│   ├── package.json              # Extension manifest & build scripts
│   └── tsconfig.json
│
├── bridge/                       # Rust — the native sidecar binary
│   ├── src/
│   │   ├── main.rs               # tokio entry point, stdin reader
│   │   ├── rpc.rs                # Protocol types (mirrors extension/src/rpc.ts)
│   │   └── handler.rs            # Dispatches requests → SessionRunner
│   └── Cargo.toml                # [[bin]] operon-vscode-bridge
│
└── README.md                     # This file
```

---

## Development

### Prerequisites

- **Rust** (stable, 2021 edition)
- **Node.js** ≥ 20
- **VS Code** ≥ 1.90

### Build the bridge binary

```bash
# From the workspace root
cargo build -p operon-vscode-bridge

# Release build (what gets bundled in the .vsix)
cargo build -p operon-vscode-bridge --release
```

The binary is output to `target/release/operon-vscode-bridge[.exe]`. Copy it to `extension/bin/` before packaging.

### Build the extension

```bash
cd vscode/extension
npm install
npm run build       # dev build with source maps
npm run build:prod  # minified production build
npm run watch       # rebuild on file change
```

### Run in VS Code (F5 debug)

1. Open the `vscode/extension` folder in VS Code
2. Copy a built bridge binary to `extension/bin/`
3. Press **F5** → VS Code opens an Extension Development Host with Operon loaded
4. Click the Operon icon in the activity bar

### Type checking

```bash
cd vscode/extension
npm run typecheck   # tsc --noEmit, zero errors expected
```

---

## Packaging (CI only)

The `.vsix` is assembled in GitHub Actions — not locally. The workflow:

1. Cross-compiles `operon-vscode-bridge` for Windows / macOS / Linux
2. Places all three binaries under `extension/bin/`
3. Runs `npx @vscode/vsce package` to produce `operon-vscode-N.N.N.vsix`
4. Uploads as a release artefact

The extension's `bin/` directory is **gitignored**. Developers build the bridge locally for F5 debugging.

---

## Session Storage

The VS Code extension shares the same session database as the GUI and TUI:

```
~/.operon/
├── config.toml              # Provider, model, and permission config
├── sessions/                # Session metadata (.json per session)
└── session-db/              # SQLite turn history (.db per session)
```

Sessions started in VS Code continue seamlessly in the GUI and vice versa.

---

<div align="center">

Built by **Luka Gray** • Part of the [Operon](https://github.com/lukagray-dev/Operon) project

</div>
