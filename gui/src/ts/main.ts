import { createNewSessionIpc } from './left-sidebar/ipc.js';
import { initSidebar, refreshSidebarContent } from './left-sidebar/sidebar.js';
import { sidebarState } from './left-sidebar/state.js';
import { initEmptyState } from './main-content/empty-state/empty-state.js';
import { initInputPanel } from './main-content/input/input.js';
import { initMessages } from './main-content/messages/messages.js';
import { initTerminal } from './main-content/terminal/terminal.js';
import { initTopbar } from './main-content/topbar/topbar.js';
import { initPermissionManager } from './main-content/permission/permission.js';
import { initRightSidebar } from './right-sidebar/right-sidebar.js';
import { openSettingsWindowIpc } from './settings/ipc.js';
import { initTitlebar } from './titlebar/titlebar.js';

window.addEventListener('DOMContentLoaded', () => {
  // Initialize Titlebar
  initTitlebar();

  // Initialize Left Sidebar
  initSidebar();

  // Auto-create a fresh general chat session on GUI startup
  createNewSessionIpc(undefined, undefined)
    .then((newId) => {
      sidebarState.selectSession(newId, null);
      refreshSidebarContent().catch(() => {});
      console.debug('[Operon GUI] Auto-created initial fresh general chat session:', newId);
    })
    .catch((err) => {
      console.warn('[Operon GUI] Failed to auto-create initial session:', err);
    });

  // Initialize Main Content Topbar
  initTopbar();

  // Initialize Bottom Terminal Panel
  initTerminal();

  // Initialize Empty State
  initEmptyState();

  // Initialize Chat Messages Stream
  initMessages();

  // Initialize Main Content Input Panel
  initInputPanel();

  // Initialize Live Permission Manager
  initPermissionManager();

  // Initialize Right Sidebar (Source Control & Git Diff)
  initRightSidebar();

  // Global Settings shortcut Ctrl+,
  window.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === ',') {
      e.preventDefault();
      await openSettingsWindowIpc();
    }
  });

  // Re-sync appearance settings and active permission state on window focus
  window.addEventListener('focus', async () => {
    try {
      const { getAppearanceSettingsIpc } = await import('./settings/appearance/ipc.js');
      const { applyGlobalFontsAndTheme } = await import('./main-content/markdown/markdown.js');
      const settings = await getAppearanceSettingsIpc();
      applyGlobalFontsAndTheme(settings);
    } catch {
      // Ignored if settings window not ready
    }

    try {
      const { syncPendingPermissionForActiveSession } = await import('./main-content/permission/permission.js');
      const { sidebarState } = await import('./left-sidebar/state.js');
      syncPendingPermissionForActiveSession(sidebarState.getActiveSessionId());
    } catch {
      // Ignore
    }
  });

  console.debug('[Operon GUI] Initialized with static TypeScript architecture.');
});
