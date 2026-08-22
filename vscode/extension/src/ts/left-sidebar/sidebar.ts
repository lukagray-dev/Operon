// ============================================================================
// Left Sidebar Overlay Drawer Controller & Dynamic DOM Renderer
// ============================================================================

import { invokeIpc } from '../shared/ipc.js';
import {
  createNewSessionIpc,
  deleteProjectIpc,
  deleteSessionIpc,
  forkSessionIpc,
  moveSessionIpc,
  openProjectPickerIpc,
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
 * Renders all projects and standalone chat items into the DOM dynamically.
 */
function renderSidebarContent(): void {
  renderProjectsSection();
  renderChatsSection();
}

function renderProjectsSection(): void {
  const container = document.getElementById('projects-items-container');
  const countBadge = document.getElementById('projects-count-badge');
  const section = document.getElementById('section-projects');
  if (!container) return;

  const projects = sidebarState.getProjects();
  if (countBadge) countBadge.textContent = String(projects.length);

  section?.classList.toggle('collapsed', sidebarState.isProjectsCollapsed());

  container.innerHTML = '';
  if (projects.length === 0) {
    container.innerHTML =
      '<div style="padding: 8px 12px; font-size: 11.5px; color: var(--text-muted, #777777);">No project folders added</div>';
    return;
  }

  projects.forEach((proj) => {
    const card = document.createElement('div');
    const isCollapsed = sidebarState.isProjectCollapsed(proj.workspace);
    card.className = `project-card ${isCollapsed ? 'collapsed' : ''}`;

    const isProjectActive = sidebarState.getActiveProjectPath() === proj.workspace;

    const header = document.createElement('div');
    header.className = `project-header ${isProjectActive ? 'active' : ''}`;
    header.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chevron-down chevron-icon proj-chevron"></span>
        <span class="ui-icon icon-sidebar-folder"></span>
        <span class="session-title-text" title="${proj.workspace}">${proj.name}</span>
      </div>
      <div class="project-header-actions">
        <button class="section-action-btn btn-proj-new-chat" title="New Chat in Project">
          <span class="ui-icon icon-sidebar-new-chat"></span>
        </button>
        <button class="section-action-btn btn-proj-delete" title="Remove Project">
          <span class="ui-icon icon-sidebar-trash"></span>
        </button>
      </div>
    `;

    header.addEventListener('click', (e) => {
      if ((e.target as HTMLElement).closest('.section-action-btn')) return;
      sidebarState.toggleProjectCollapsed(proj.workspace);
    });

    header.querySelector('.btn-proj-new-chat')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const newId = await createNewSessionIpc(undefined, proj.workspace);
      sidebarState.selectSession(newId, proj.workspace);
      await refreshSidebarContent();
      closeSidebar();
    });

    header.querySelector('.btn-proj-delete')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (confirm(`Remove project "${proj.name}" from your workspace list?`)) {
        await deleteProjectIpc(proj.workspace);
        if (sidebarState.getActiveProjectPath() === proj.workspace) {
          sidebarState.selectSession(null, null);
        }
        await refreshSidebarContent();
      }
    });

    card.appendChild(header);

    if (proj.conversations.length > 0) {
      const convList = document.createElement('div');
      convList.className = 'project-conversations';

      proj.conversations.forEach((conv) => {
        const item = document.createElement('div');
        const isActive = sidebarState.getActiveSessionId() === conv.id;
        item.className = `session-item ${isActive ? 'active' : ''}`;
        item.innerHTML = `
          <div class="session-item-left">
            <span class="session-title-text" title="${conv.title}">${conv.title}</span>
          </div>
          <button class="item-more-btn" title="Options">
            <span class="ui-icon icon-sidebar-more-vertical"></span>
          </button>
        `;

        item.addEventListener('click', () => {
          sidebarState.selectSession(conv.id, proj.workspace);
          closeSidebar();
        });

        const moreBtn = item.querySelector<HTMLButtonElement>('.item-more-btn');
        moreBtn?.addEventListener('click', (e) => {
          e.stopPropagation();
          showConversationContextMenu(e, moreBtn, conv, proj.workspace);
        });

        convList.appendChild(item);
      });

      card.appendChild(convList);
    }

    container.appendChild(card);
  });
}

function renderChatsSection(): void {
  const container = document.getElementById('chats-items-container');
  const countBadge = document.getElementById('chats-count-badge');
  const section = document.getElementById('section-chats');
  if (!container) return;

  const chats = sidebarState.getChats();
  if (countBadge) countBadge.textContent = String(chats.length);

  section?.classList.toggle('collapsed', sidebarState.isChatsCollapsed());

  container.innerHTML = '';
  if (chats.length === 0) {
    container.innerHTML =
      '<div style="padding: 8px 12px; font-size: 11.5px; color: var(--text-muted, #777777);">No conversations yet</div>';
    return;
  }

  chats.forEach((chat) => {
    const item = document.createElement('div');
    const isActive = sidebarState.getActiveSessionId() === chat.id;
    item.className = `session-item ${isActive ? 'active' : ''}`;
    item.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chats"></span>
        <span class="session-title-text" title="${chat.title}">${chat.title}</span>
      </div>
      <button class="item-more-btn" title="Options">
        <span class="ui-icon icon-sidebar-more-vertical"></span>
      </button>
    `;

    item.addEventListener('click', () => {
      sidebarState.selectSession(chat.id, null);
      closeSidebar();
    });

    const moreBtn = item.querySelector<HTMLButtonElement>('.item-more-btn');
    moreBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      showConversationContextMenu(e, moreBtn, chat, undefined);
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
    <button class="context-menu-item" id="ctx-move">
      <span class="ui-icon icon-sidebar-folder-input"></span>
      <span>Move to...</span>
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

  // Move to
  menu.querySelector('#ctx-move')?.addEventListener('click', async (ev) => {
    ev.stopPropagation();
    dismissContextMenu();
    const folder = await openProjectPickerIpc();
    if (folder) {
      await moveSessionIpc(conv.id, folder);
      await refreshSidebarContent();
    }
  });

  // Delete
  menu.querySelector('#ctx-delete')?.addEventListener('click', async (ev) => {
    ev.stopPropagation();
    dismissContextMenu();
    if (confirm(`Delete conversation "${conv.title}"?`)) {
      await deleteSessionIpc(conv.id);
      if (sidebarState.getActiveSessionId() === conv.id) {
        sidebarState.selectSession(null, null);
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
  const btnAddProject = document.getElementById('btn-add-project');
  const btnAddGeneralChat = document.getElementById('btn-add-general-chat');
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
      }
    }
  });

  // New Chat action
  btnNewChat?.addEventListener('click', async () => {
    const newId = await createNewSessionIpc(undefined, undefined);
    sidebarState.selectSession(newId, null);
    await refreshSidebarContent();
    closeSidebar();
  });

  // Add Project Folder action
  btnAddProject?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const folder = await openProjectPickerIpc();
    if (folder) {
      const newId = await createNewSessionIpc(undefined, folder);
      sidebarState.selectSession(newId, folder);
      await refreshSidebarContent();
      closeSidebar();
    }
  });

  // Add General Chat action
  btnAddGeneralChat?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const newId = await createNewSessionIpc(undefined, undefined);
    sidebarState.selectSession(newId, null);
    await refreshSidebarContent();
    closeSidebar();
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

  document.getElementById('header-chats')?.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).closest('.section-action-btn')) return;
    sidebarState.toggleChatsCollapsed();
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
