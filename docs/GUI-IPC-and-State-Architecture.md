# GUI IPC & State Architecture

This document defines the modular domain-driven architecture for frontend state and Tauri IPC communication in Operon GUI.

---

## 1. Core Architectural Principle

To prevent monolithic bottleneck files and ensure high cohesion, state management and IPC wrappers are **partitioned by domain** across both TypeScript (`gui/src/ts/`) and Rust (`gui/src-tauri/src/`).

---

## 2. Directory Layout & Symmetrical Mapping

```text
gui/src/ts/                               gui/src-tauri/src/
├── left-sidebar/                         ├── left-sidebar/
│   ├── state.ts    (Session/project)     │   ├── mod.rs
│   ├── ipc.ts      (Session commands)    │   └── session.rs
│   └── sidebar.ts                        │
├── main-content/                         ├── main-content/
│   ├── state.ts    (Messages/tokens)     │   ├── mod.rs
│   ├── ipc.ts      (Prompt & streams)    │   └── agent.rs
│   └── chat.ts                           │
├── right-sidebar/                        ├── right-sidebar/
│   ├── state.ts    (Git diffs/graph)     │   ├── mod.rs
│   ├── ipc.ts      (Git operations)      │   └── git.rs
│   └── git-panel.ts                      │
├── settings/                             ├── settings/
│   ├── state.ts    (Config & keys)       │   ├── mod.rs
│   ├── ipc.ts      (Save & channels)     │   └── config.rs
│   └── settings.ts                       │
└── shared/                               └── shared/
    ├── ipc.ts      (Core invoke wrapper)     ├── mod.rs
    ├── store.ts    (Reactive store primitive)└── state.rs (Global AppState)
    └── types.ts    (DTO interfaces)
```

---

## 3. Domain State & IPC Guidelines

1. **Feature Scoping**:
   - Each domain folder owns its state class/signals and typed IPC calls.
   - Example: `main-content/ipc.ts` exposes `sendPrompt()`, `cancelExecution()`, and `onTokenStream()`.
2. **Shared Layer**:
   - `shared/ipc.ts` contains only the low-level generic `invokeIpc<T>()` and `listenEvent<T>()` wrappers (~30 LOC).
   - `shared/types.ts` contains shared data-transfer objects (DTOs) mirroring Rust backend types.
3. **Isolated Reactivity**:
   - Updates in one domain (e.g. streaming tokens in `main-content`) trigger updates only in that domain's subscriber components.
4. **File Size Target**:
   - Every file remains lightweight and maintainable (~100–300 LOC).
