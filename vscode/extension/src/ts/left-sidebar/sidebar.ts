// ============================================================================
// Left Sidebar Overlay Drawer Controller & Dynamic DOM Renderer
// ============================================================================

import { todoPanelState } from '../right-sidebar/state.js';
import { invokeIpc, listenIpcEvent } from '../shared/ipc.js';
import {
  createNewSessionIpc,
  deleteSessionIpc,
  forkSessionIpc,
  querySidebarData,
  renameSessionIpc,
} from './ipc.js';
import { sidebarState } from './state.js';
import type { SidebarConversation } from './types.js';

let leftSidebarEl: HTMLElement | null = null;
let sidebarBackdropEl: HTMLElement | null = null;
let activeContextMenu: HTMLElement | null = null;

/**
 * Opens the left sidebar overlay drawer and shows the dimmed backdrop.
 */
export function openSidebar(): void {
  if (!leftSidebarEl) leftSidebarEl = document.getElementById('left-sidebar');
  if (!sidebarBackdropEl) sidebarBackdropEl = document.getElementById('sidebar-backdrop');

  leftSidebarEl?.classList.add('open');
  sidebarBackdropEl?.classList.add('visible');

  const btnToggleSidebar = document.getElementById('btn-toggle-sidebar');
  const toggleIcon = btnToggleSidebar?.querySelector('.ui-icon');
  if (toggleIcon) {
    toggleIcon.className = 'ui-icon icon-titlebar-sidebar-opened';
  }
}

/**
 * Closes the left sidebar overlay drawer and hides the backdrop.
 */
export function closeSidebar(): void {
  if (!leftSidebarEl) leftSidebarEl = document.getElementById('left-sidebar');
  if (!sidebarBackdropEl) sidebarBackdropEl = document.getElementById('sidebar-backdrop');

  leftSidebarEl?.classList.remove('open');
  sidebarBackdropEl?.classList.remove('visible');
  dismissContextMenu();

  const btnToggleSidebar = document.getElementById('btn-toggle-sidebar');
  const toggleIcon = btnToggleSidebar?.querySelector('.ui-icon');
  if (toggleIcon) {
    toggleIcon.className = 'ui-icon icon-titlebar-sidebar-closed';
  }
}

/**
 * Toggles the open / closed state of the left sidebar drawer.
 */
export function toggleSidebar(): void {
  if (!leftSidebarEl) leftSidebarEl = document.getElementById('left-sidebar');
  if (leftSidebarEl?.classList.contains('open')) {
    closeSidebar();
  } else {
    openSidebar();
  }
}

/**
 * Dismisses any currently open three-dots conversation context menu.
 */
export function dismissContextMenu(): void {
  if (activeContextMenu) {
    activeContextMenu.remove();
    activeContextMenu = null;
  }
  document.querySelectorAll('.item-more-btn.active').forEach((btn) => {
    btn.classList.remove('active');
  });
}

/**
 * Queries real session data from the bridge backend and updates sidebar state.
 */
export async function refreshSidebarContent(): Promise<void> {
  const query = sidebarState.getSearchQuery();
  try {
    const data = await querySidebarData(query);
    sidebarState.setSidebarData(data);
  } catch (err) {
    console.warn('[Sidebar] Failed to query sidebar data:', err);
  }
}

/**
 * Renders conversation items for the active workspace project into the DOM dynamically.
 */
function renderSidebarContent(): void {
  const container = document.getElementById('projects-items-container');
  const countBadge = document.getElementById('projects-count-badge');
  const titleEl = document.getElementById('sidebar-project-title');
  const section = document.getElementById('section-projects');
  if (!container) return;

  const activeWorkspace = sidebarState.getActiveProjectPath();
  const activeProject = sidebarState.getActiveProject();

  // Determine friendly project name
  let projectName = 'Conversations';
  if (activeProject && activeProject.name) {
    projectName = activeProject.name;
  } else if (activeWorkspace) {
    const parts = activeWorkspace.replace(/[/\\]+/g, '/').split('/');
    projectName = parts[parts.length - 1] || 'Project';
  }

  if (titleEl) {
    titleEl.textContent = projectName;
    titleEl.setAttribute('title', activeWorkspace || projectName);
  }

  const conversations = activeProject ? activeProject.conversations : [];
  if (countBadge) countBadge.textContent = String(conversations.length);

  section?.classList.toggle('collapsed', sidebarState.isProjectsCollapsed());

  container.innerHTML = '';
  if (conversations.length === 0) {
    container.innerHTML =
      '<div style="padding: 16px 12px; font-size: 11.5px; color: var(--text-muted, #777777); text-align: center; line-height: 1.4;">No conversations in this project yet.<br/><span style="opacity: 0.7;">Click "New conversation" to start.</span></div>';
    return;
  }

  conversations.forEach((conv) => {
    const item = document.createElement('div');
    const isActive = sidebarState.getActiveSessionId() === conv.id;
    item.className = `session-item ${isActive ? 'active' : ''}`;
    item.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chats"></span>
        <span class="session-title-text" title="${conv.title}">${conv.title}</span>
      </div>
      <button class="item-more-btn" title="Options">
        <span class="ui-icon icon-sidebar-more-vertical"></span>
      </button>
    `;

    item.addEventListener('click', () => {
      sidebarState.selectSession(conv.id, activeWorkspace);
      closeSidebar();
    });

    const moreBtn = item.querySelector<HTMLButtonElement>('.item-more-btn');
    moreBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      showConversationContextMenu(e, moreBtn, conv, activeWorkspace || undefined);
    });

    container.appendChild(item);
  });
}

/**
 * Displays the floating three-dots context menu next to the clicked session item.
 */
function showConversationContextMenu(
  e: MouseEvent,
  targetBtn: HTMLElement,
  conv: SidebarConversation,
  projectPath?: string
): void {
  e.stopPropagation();
  dismissContextMenu();

  targetBtn.classList.add('active');
  const rect = targetBtn.getBoundingClientRect();

  const menu = document.createElement('div');
  menu.className = 'session-context-menu';

  const top = Math.min(window.innerHeight - 200, rect.bottom + 4);
  const left = Math.min(window.innerWidth - 150, rect.left - 100);
  menu.style.top = `${Math.max(8, top)}px`;
  menu.style.left = `${Math.max(8, left)}px`;

  menu.innerHTML = `
    <button class="context-menu-item" id="ctx-rename">
      <span class="ui-icon icon-sidebar-pencil"></span>
      <span>Rename</span>
    </button>
    <button class="context-menu-item" id="ctx-fork">
      <span class="ui-icon icon-sidebar-fork"></span>
      <span>Fork</span>
    </button>
    <div class="context-menu-separator"></div>
    <button class="context-menu-item danger" id="ctx-delete">
      <span class="ui-icon icon-sidebar-trash"></span>
      <span>Delete</span>
    </button>
  `;

  document.body.appendChild(menu);
  activeContextMenu = menu;

  // Rename
  menu.querySelector('#ctx-rename')?.addEventListener('click', async (ev) => {
    ev.stopPropagation();
    dismissContextMenu();
    const newTitle = prompt('Enter new conversation title:', conv.title);
    if (newTitle && newTitle.trim() && newTitle !== conv.title) {
      await renameSessionIpc(conv.id, newTitle.trim());
      await refreshSidebarContent();
    }
  });

  // Fork
  menu.querySelector('#ctx-fork')?.addEventListener('click', async (ev) => {
    ev.stopPropagation();
    dismissContextMenu();
    const forkedId = await forkSessionIpc(conv.id);
    sidebarState.selectSession(forkedId, projectPath || null);
    await refreshSidebarContent();
    closeSidebar();
  });

  // Delete
  menu.querySelector('#ctx-delete')?.addEventListener('click', async (ev) => {
    ev.stopPropagation();
    dismissContextMenu();
    if (confirm(`Delete conversation "${conv.title}"?`)) {
      await deleteSessionIpc(conv.id);
      if (sidebarState.getActiveSessionId() === conv.id) {
        sidebarState.selectSession(null, projectPath || null);
      }
      await refreshSidebarContent();
    }
  });
}

/**
 * Initializes the sidebar drawer DOM elements, event listeners, and accordions.
 */
export function initSidebar(): void {
  leftSidebarEl = document.getElementById('left-sidebar');
  sidebarBackdropEl = document.getElementById('sidebar-backdrop');
  const btnCloseSidebar = document.getElementById('btn-close-sidebar');
  const btnSidebarSettings = document.getElementById('btn-sidebar-settings');
  const btnNewChat = document.getElementById('btn-new-chat');
  const btnAddChatHeader = document.getElementById('btn-add-chat-header');
  const searchInput = document.getElementById('sidebar-search-input') as HTMLInputElement | null;
  const btnSearchClear = document.getElementById('btn-search-clear');

  // Close when clicking ✕ close button or dimmed backdrop
  btnCloseSidebar?.addEventListener('click', closeSidebar);
  sidebarBackdropEl?.addEventListener('click', closeSidebar);

  // Close on Escape key press
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (activeContextMenu) {
        dismissContextMenu();
      } else if (leftSidebarEl?.classList.contains('open')) {
        closeSidebar();
      } else if (todoPanelState.getIsOpen()) {
        todoPanelState.setIsOpen(false);
      }
    }
  });

  // New Chat action (strictly project-scoped in VS Code)
  const handleNewConversation = async () => {
    const wsPath = sidebarState.getActiveProjectPath() || null;
    const newId = await createNewSessionIpc(undefined, wsPath || undefined);
    sidebarState.selectSession(newId, wsPath);
    await refreshSidebarContent();
    closeSidebar();
  };

  btnNewChat?.addEventListener('click', handleNewConversation);
  btnAddChatHeader?.addEventListener('click', (e) => {
    e.stopPropagation();
    handleNewConversation();
  });

  // Listen to external new-session event
  listenIpcEvent<{ workspacePath?: string }>('new-session', async (payload) => {
    const wsPath = payload?.workspacePath || sidebarState.getActiveProjectPath() || null;
    const newId = await createNewSessionIpc(undefined, wsPath || undefined);
    sidebarState.selectSession(newId, wsPath);
    await refreshSidebarContent();
  });

  // Settings button click: closes drawer and opens settings editor tab
  btnSidebarSettings?.addEventListener('click', async () => {
    closeSidebar();
    await invokeIpc('open_settings_window');
  });

  // Section Accordion Header Collapse / Expand
  document.getElementById('header-projects')?.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).closest('.section-action-btn')) return;
    sidebarState.toggleProjectsCollapsed();
  });

  // Search Input
  if (searchInput && btnSearchClear) {
    searchInput.addEventListener('input', () => {
      btnSearchClear.classList.toggle('visible', searchInput.value.length > 0);
      sidebarState.setSearchQuery(searchInput.value.trim());
      refreshSidebarContent();
    });

    btnSearchClear.addEventListener('click', () => {
      searchInput.value = '';
      btnSearchClear.classList.remove('visible');
      sidebarState.setSearchQuery('');
      refreshSidebarContent();
      searchInput.focus();
    });
  }

  // Dismiss context menu on outside click
  window.addEventListener('click', (e) => {
    if (activeContextMenu && !(e.target as HTMLElement).closest('.session-context-menu')) {
      dismissContextMenu();
    }
  });

  // Subscribe to state changes to re-render sidebar
  sidebarState.subscribe(() => {
    renderSidebarContent();
  });

  // Initial load
  refreshSidebarContent();
  console.log('[Operon Sidebar] Initialized with real backend data.');
}
