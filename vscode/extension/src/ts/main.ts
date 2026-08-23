// ============================================================================
// Application Root Coordinator for VS Code Webview
//
// Hey friend! This is the root coordinator running inside our Webview DOM.
// It delegates responsibilities to dedicated component controllers:
//
// 1. Left Sidebar Overlay Drawer  -> ./left-sidebar/sidebar.ts
// 2. Main Content Topbar          -> ./main-content/topbar/topbar.ts
// 3. Splash Empty State           -> ./main-content/empty-state/empty-state.ts
// 4. Chat Messages Stream         -> ./main-content/messages/messages.ts
// 5. Floating Input Panel         -> ./main-content/input/input.ts
// 6. Right Sidebar (Tasks Panel)  -> ./right-sidebar/todo-panel.ts
// 7. Global Settings Shortcuts    -> ./shared/ipc.ts
// ============================================================================

import { createNewSessionIpc } from './left-sidebar/ipc.js';
import { closeSidebar, initSidebar, refreshSidebarContent } from './left-sidebar/sidebar.js';
import { sidebarState } from './left-sidebar/state.js';
import { initEmptyState } from './main-content/empty-state/empty-state.js';
import { initInputPanel } from './main-content/input/input.js';
import { initMessages } from './main-content/messages/messages.js';
import { initPermissionManager } from './main-content/permission/permission.js';
import { initTopbar, refreshTopbar } from './main-content/topbar/topbar.js';
import { todoPanelState } from './right-sidebar/state.js';
import { renderTodoPanel } from './right-sidebar/todo-panel.js';
import { invokeIpc, listenIpcEvent } from './shared/ipc.js';

interface WorkspaceInfo {
  hasWorkspace: boolean;
  workspacePath: string | null;
  workspaceName: string | null;
}

/**
 * Toggles visibility between the "No Workspace Opened" disclaimer screen
 * and the main interactive chat pane based on IDE workspace availability.
 */
function updateWorkspaceView(ws: WorkspaceInfo): void {
  const noWorkspaceEl = document.getElementById('no-workspace-view');
  const contentPaneEl = document.getElementById('content-pane');

  if (!ws.hasWorkspace || !ws.workspacePath) {
    console.log('[Operon Webview] No workspace folder detected in IDE. Showing disclaimer view.');
    noWorkspaceEl?.classList.remove('hidden');
    contentPaneEl?.classList.add('hidden');
    closeSidebar();
    sidebarState.setActiveProjectPath(null);
    sidebarState.setActiveSessionId(null);
  } else {
    console.log(`[Operon Webview] Active workspace folder connected: ${ws.workspacePath}`);
    noWorkspaceEl?.classList.add('hidden');
    contentPaneEl?.classList.remove('hidden');
    const wsPath = ws.workspacePath;
    sidebarState.setActiveProjectPath(wsPath);

    // Auto-create a fresh project-scoped session if no session is active yet
    if (!sidebarState.getActiveSessionId()) {
      createNewSessionIpc(undefined, wsPath)
        .then((newId) => {
          sidebarState.selectSession(newId, wsPath);
          refreshTopbar().catch(() => {});
          refreshSidebarContent().catch(() => {});
          console.log(`[Operon Webview] Auto-created initial fresh project session: ${newId}`);
        })
        .catch((err) => {
          console.warn('[Operon Webview] Failed to auto-create initial session:', err);
          refreshTopbar().catch(() => {});
          refreshSidebarContent().catch(() => {});
        });
    } else {
      refreshTopbar().catch(() => {});
      refreshSidebarContent().catch(() => {});
    }
  }
}

function initApp(): void {
  console.log('[Operon Webview] Initializing modular UI layout...');

  // 1. Initialize Left Sidebar Overlay Drawer
  initSidebar();

  // 2. Initialize Main Content Topbar
  initTopbar();

  // 3. Initialize Splash Empty State
  initEmptyState();

  // 4. Initialize Chat Messages Stream
  initMessages();

  // 4b. Initialize Permission Manager
  initPermissionManager().catch((e) => console.warn('[Main] Permission manager init error:', e));

  // 5. Initialize Floating Input Card
  initInputPanel();

  // 6. Initialize Right Sidebar Task Panel
  const rightSidebarEl = document.getElementById('right-sidebar');
  if (rightSidebarEl) {
    todoPanelState.subscribe(() => {
      const isOpen = todoPanelState.getIsOpen();
      if (isOpen) {
        rightSidebarEl.classList.add('visible');
        renderTodoPanel(rightSidebarEl);
      } else {
        rightSidebarEl.classList.remove('visible');
      }
    });
  }

  // 7. Initialize "Open Folder" button on the No Workspace disclaimer screen
  const btnOpenWorkspace = document.getElementById('btn-open-workspace');
  btnOpenWorkspace?.addEventListener('click', async () => {
    try {
      await invokeIpc('open_workspace_folder');
    } catch (err) {
      console.error('[Main] Failed to open workspace folder:', err);
    }
  });

  // 8. Query initial workspace state from VS Code Extension Host
  invokeIpc<WorkspaceInfo>('get_workspace_info')
    .then((ws) => {
      if (ws) {
        updateWorkspaceView(ws);
      }
    })
    .catch((err) => console.warn('[Main] Failed to get initial workspace info:', err));

  // 9. Subscribe to live workspace change notifications
  listenIpcEvent<WorkspaceInfo>('operon://workspace-changed', (ws) => {
    if (ws) {
      updateWorkspaceView(ws);
    }
  }).catch((err) => console.warn('[Main] Failed to listen to workspace changes:', err));

  // 10. Global Settings shortcut (Ctrl+, or Cmd+,)
  window.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === ',') {
      e.preventDefault();
      await invokeIpc('open_settings_window');
    }
  });

  console.log('[Operon Webview] UI layout initialized successfully.');
}

if (document.readyState === 'loading') {
  window.addEventListener('DOMContentLoaded', initApp);
} else {
  initApp();
}
