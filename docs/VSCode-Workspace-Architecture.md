# VS Code Extension Workspace & Lifecycle Architecture

This document defines the architecture, design principles, and implementation details of the workspace lifecycle, permission auto-registration, and project-scoped conversation management in the Operon VS Code Extension (`vscode/`).

---

## 1. Background & Context

In the standalone Operon desktop application (`gui/`), users can open projects via the application menu bar or manage standalone general chat sessions. In general chat sessions, `operon-snapshot` defaults to `~/.operon/workspace/` as its workspace directory.

However, inside an IDE such as Visual Studio Code, operating on an arbitrary default global workspace without an active project folder poses distinct architectural and safety problems:
1. **Safety & Policy Isolation**: An AI coding agent running inside an IDE must never operate or execute filesystem/terminal actions in an unconfigured default directory.
2. **Directory-Scoped Permissions**: Operon's security architecture enforces directory permission boundaries defined in `policy.directories`. Without an active project folder, tool calls (e.g., file search, file read, write, edit) cannot be scoped safely.
3. **User Intent Alignment**: Users open an extension in an IDE specifically to inspect, refactor, test, and write code for the currently opened project.

To resolve these constraints, the Operon VS Code Extension enforces a strict **Workspace-First Lifecycle**.

---

## 2. Architecture Overview

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        VS Code Extension Host                          │
│                                                                        │
│   vscode.workspace.workspaceFolders                                    │
│        │                                                               │
│   getActiveWorkspaceInfo() ────────┐                                   │
│        │                           │                                   │
│   syncActiveWorkspace...()         │                                   │
│        │                           │                                   │
│   onDidChangeWorkspaceFolders      │                                   │
│        │                           │                                   │
│        ▼                           ▼                                   │
│   ┌──────────────────────────┐    ┌────────────────────────────────┐   │
│   │ Native Rust Bridge Client│    │ Webview IPC Dispatcher         │   │
│   │ (operon-vscode-bridge)   │    │ (postMessage / event bus)      │   │
│   └────────────┬─────────────┘    └──────────────┬─────────────────┘   │
└────────────────┼─────────────────────────────────┼─────────────────────┘
                 │ (add_allowed_directory)         │ (operon://workspace-changed)
                 ▼                                 ▼
    ┌───────────────────────────┐    ┌───────────────────────────────────┐
    │  Operon Core Policy       │    │  Webview DOM Coordinator          │
    │  policy.directories       │    │  (src/ts/main.ts)                 │
    │  [allowed list updated]   │    │                                   │
    └───────────────────────────┘    │  ┌─────────────────────────────┐  │
                                     │  │ Has Workspace Folder?       │  │
                                     │  └───┬─────────────────────┬───┘  │
                                     │      │ No                  │ Yes  │
                                     │      ▼                     ▼      │
                                     │  [#no-workspace-view]  [#content- │
                                     │  - Disclaimer screen    pane]     │
                                     │  - "Open Folder" btn   - Active   │
                                     │    (showOpenDialog)      project  │
                                     │                          chats    │
                                     └───────────────────────────────────┘
```

---

## 3. Core Subsystems

### 3.1. Extension Host Workspace Detection & Handshake

The extension host (`vscode/extension/extension.js`) acts as the bridge between VS Code's environment and Operon:

1. **Workspace Inspection**:
   ```javascript
   function getActiveWorkspaceInfo() {
     const folders = vscode.workspace.workspaceFolders;
     if (!folders || folders.length === 0) {
       return { hasWorkspace: false, workspacePath: null, workspaceName: null };
     }
     const primary = folders[0];
     return {
       hasWorkspace: true,
       workspacePath: primary.uri.fsPath,
       workspaceName: primary.name,
     };
   }
   ```

2. **Allowed Directories Synchronization**:
   Whenever an active workspace folder is identified upon extension activation or when folders change via `vscode.workspace.onDidChangeWorkspaceFolders`, the extension host invokes `add_allowed_directory` on the native bridge:
   ```javascript
   async function syncActiveWorkspaceToAllowedDirectories() {
     const wsInfo = getActiveWorkspaceInfo();
     if (wsInfo.hasWorkspace && wsInfo.workspacePath && bridgeClient) {
       try {
         await bridgeClient.invoke('add_allowed_directory', { path: wsInfo.workspacePath });
       } catch (err) {
         console.warn('[Operon] Auto-register allowed directory failed:', err);
       }
     }
   }
   ```

3. **Real-time Event Broadcasting**:
   When folders are opened, added, or removed, the extension host broadcasts an `operon://workspace-changed` event to the webview.

---

### 3.2. Disclaimer View & Interactive Folder Picker

When no folder is currently opened in VS Code:
- The webview hides `#content-pane` and displays `#no-workspace-view`.
- The disclaimer informs the user that a project folder is required for the agent to inspect files, run tests, and track tasks.
- Clicking the **Open Folder** button invokes the `open_workspace_folder` IPC command:
  ```javascript
  case 'open_workspace_folder': {
    const uri = await vscode.window.showOpenDialog({
      canSelectFiles: false,
      canSelectFolders: true,
      canSelectMany: false,
      openLabel: 'Open Folder',
    });
    if (uri && uri[0]) {
      await vscode.commands.executeCommand('vscode.openFolder', uri[0], { forceNewWindow: false });
    }
    return null;
  }
  ```
- As soon as the folder is selected and loaded into the window, the webview receives `operon://workspace-changed`, hides the disclaimer, and renders the interactive chat environment.

---

### 3.3. Project-Scoped Sidebar & Conversation Management

Unlike the desktop GUI where general and project-specific conversations coexist in separate sections, the VS Code extension streamlines the sidebar into a single **Project Conversations** view:

1. **Active Project Filtering**:
   - `SidebarStateManager` tracks `activeProjectPath`.
   - `renderSidebarContent()` filters conversation history to only show records matching the active project path.
2. **Project-Scoped New Sessions**:
   - The top `New conversation` button (`#btn-new-chat`) and `Ctrl+N` shortcut automatically pass `workspacePath = sidebarState.getActiveProjectPath()` to `createNewSessionIpc()`.
   - This ensures all newly created chats are attached to the current project from the very first turn.

---

### 3.4. Cross-Platform Windows & UNC Path Normalization

Path representation varies between Windows components:
- VS Code `uri.fsPath` returns standard DOS drive paths (e.g. `d:\Operon`).
- Rust's `std::fs::canonicalize()` prefixes Windows paths with UNC prefix (e.g. `\\?\D:\Operon`).
- Drive letters can vary in casing between subsystems (`d:` vs `D:`).

To ensure deterministic equality across Rust and TypeScript:

1. **Rust Bridge Normalization (`vscode/bridge/src/left-sidebar/session.rs`)**:
   ```rust
   pub fn normalize_path_str(p: &str) -> String {
       let clean = clean_unc_path(p.to_string());
       let mut normalized = clean.replace('\\', "/");
       while normalized.ends_with('/') && normalized.len() > 1 {
           normalized.pop();
       }
       #[cfg(windows)]
       {
           normalized.to_lowercase()
       }
       #[cfg(not(windows))]
       {
           normalized
       }
   }

   pub fn paths_match(a: &str, b: &str) -> bool {
       normalize_path_str(a) == normalize_path_str(b)
   }
   ```

2. **TypeScript Webview Normalization (`vscode/extension/src/ts/left-sidebar/state.ts`)**:
   ```typescript
   export function normalizePath(p: string | null | undefined): string {
     if (!p) return '';
     let norm = p.trim().replace(/^\\\\\?\\UNC\\/i, '\\\\').replace(/^\\\\\?\\[a-zA-Z]:/i, (m) => m.slice(4));
     norm = norm.replace(/\\/g, '/');
     while (norm.endsWith('/') && norm.length > 1) {
       norm = norm.slice(0, -1);
     }
     return norm.toLowerCase();
   }

   export function pathsMatch(a: string | null | undefined, b: string | null | undefined): bool {
     return normalizePath(a) === normalizePath(b);
   }
   ```

---

## 4. Summary

By isolating workspace enforcement to the VS Code extension boundary (`vscode/extension` and `vscode/bridge`), core `operon-rs`, `gui`, and `tui` packages remain completely unaffected while the VS Code extension delivers a safe, project-scoped developer experience.
