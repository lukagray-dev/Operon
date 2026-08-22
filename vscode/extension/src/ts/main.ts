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

import { initSidebar } from './left-sidebar/sidebar.js';
import { initEmptyState } from './main-content/empty-state/empty-state.js';
import { initInputPanel } from './main-content/input/input.js';
import { initMessages } from './main-content/messages/messages.js';
import { initPermissionManager } from './main-content/permission/permission.js';
import { initTopbar } from './main-content/topbar/topbar.js';
import { todoPanelState } from './right-sidebar/state.js';
import { refreshTodoPanel, renderTodoPanel } from './right-sidebar/todo-panel.js';
import { invokeIpc } from './shared/ipc.js';

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
    todoPanelState.subscribe(async () => {
      const isOpen = todoPanelState.getIsOpen();
      if (isOpen) {
        rightSidebarEl.classList.add('visible');
        renderTodoPanel(rightSidebarEl);
        await refreshTodoPanel();
      } else {
        rightSidebarEl.classList.remove('visible');
      }
    });
  }

  // 7. Global Settings shortcut (Ctrl+, or Cmd+,)
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
