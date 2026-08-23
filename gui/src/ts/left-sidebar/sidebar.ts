import { openSettingsWindowIpc } from '../settings/ipc.js';
import { listenIpcEvent } from '../shared/ipc.js';
import { appState } from '../shared/state.js';
import { hasPendingPermission, onPendingPermissionsChange } from '../main-content/permission/permission.js';
import {
  createNewSessionIpc,
  deleteProjectIpc,
  deleteSessionIpc,
  forkSessionIpc,
  moveSessionIpc,
  openProjectPickerIpc,
  querySidebarData,
  queryTelegramContactsIpc,
  queryWhatsAppContactsIpc,
  renameSessionIpc,
} from './ipc.js';
import { showConfirmDialog, showPromptDialog } from '../shared/dialog.js';
import { initSidebarUpdater } from './updater.js';
import { sidebarState } from './state.js';
import type { SidebarConversation } from './types.js';

let activeContextMenu: HTMLElement | null = null;

export function initSidebar(): void {
  setupSidebarCollapseSync();
  setupTopActions();
  setupSearch();
  setupSectionToggles();
  setupResizeHandle();
  setupBottomActions();
  initSidebarUpdater();

  // Close context menu on outside click or window resize
  window.addEventListener('click', () => {
    dismissContextMenu();
  });

  window.addEventListener('resize', () => {
    dismissContextMenu();
  });

  // Initial load
  refreshSidebarContent();

  // Re-render when sidebar state changes
  sidebarState.subscribe(() => {
    renderSidebarContent();
  });

  // Re-render when permission status changes on any session
  onPendingPermissionsChange(() => {
    renderSidebarContent();
  });

  // Hot-reload only channel contact lists when notify watcher detects channel session changes on disk
  listenIpcEvent<string[]>('sessions-changed', async () => {
    await refreshChannelContactsOnly();
  });
}

export async function refreshChannelContactsOnly(): Promise<void> {
  const [whatsapp, telegram] = await Promise.all([
    queryWhatsAppContactsIpc(),
    queryTelegramContactsIpc(),
  ]);
  sidebarState.setChannelContacts(whatsapp, telegram);
}

export function dismissContextMenu(): void {
  if (activeContextMenu) {
    activeContextMenu.remove();
    activeContextMenu = null;
  }
}

function setupSectionToggles(): void {
  document.getElementById('header-projects')?.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).closest('.section-action-btn')) return;
    sidebarState.toggleProjectsCollapsed();
  });

  document.getElementById('header-chats')?.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).closest('.section-action-btn')) return;
    sidebarState.toggleChatsCollapsed();
  });

  document.getElementById('btn-add-general-chat')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const newId = await createNewSessionIpc(undefined, undefined);
    sidebarState.selectSession(newId, null);
    await refreshSidebarContent();
  });

  document.getElementById('header-whatsapp')?.addEventListener('click', () => {
    sidebarState.toggleWhatsAppCollapsed();
  });

  document.getElementById('header-telegram')?.addEventListener('click', () => {
    sidebarState.toggleTelegramCollapsed();
  });
}

export async function refreshSidebarContent(): Promise<void> {
  const query = sidebarState.getSearchQuery();
  const [data, whatsapp, telegram] = await Promise.all([
    querySidebarData(query),
    queryWhatsAppContactsIpc(),
    queryTelegramContactsIpc(),
  ]);

  sidebarState.setSidebarData(data);
  sidebarState.setChannelContacts(whatsapp, telegram);
}

function setupSidebarCollapseSync(): void {
  const sidebar = document.getElementById('left-sidebar');
  if (!sidebar) return;

  appState.subscribe(() => {
    const isOpen = appState.getSidebarOpen();
    sidebar.classList.toggle('collapsed', !isOpen);
  });
}

function setupTopActions(): void {
  // New Chat (Always creates a new general chat session)
  document.getElementById('btn-new-chat')?.addEventListener('click', async () => {
    const newId = await createNewSessionIpc(undefined, undefined);
    sidebarState.selectSession(newId, null);
    await refreshSidebarContent();
    console.debug('[Sidebar] New general chat created:', newId);
  });

  // Plugins
  document.getElementById('btn-plugins')?.addEventListener('click', () => {
    console.debug('[Sidebar] Plugins clicked');
  });

  // Add Project
  document.getElementById('btn-add-project')?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const picked = await openProjectPickerIpc();
    if (picked) {
      sidebarState.setActiveProjectPath(picked);
      await refreshSidebarContent();
    }
  });
}

function setupSearch(): void {
  const input = document.getElementById('sidebar-search-input') as HTMLInputElement | null;
  const clearBtn = document.getElementById('btn-search-clear');

  let debounceTimer: number | undefined;

  input?.addEventListener('input', () => {
    const val = input.value;
    clearBtn?.classList.toggle('visible', val.length > 0);

    clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(async () => {
      sidebarState.setSearchQuery(val);
      await refreshSidebarContent();
    }, 200);
  });

  clearBtn?.addEventListener('click', async () => {
    if (input) {
      input.value = '';
      clearBtn.classList.remove('visible');
      sidebarState.setSearchQuery('');
      await refreshSidebarContent();
    }
  });
}

function setupBottomActions(): void {
  document.getElementById('btn-sidebar-settings')?.addEventListener('click', async () => {
    await openSettingsWindowIpc();
  });
}

function setupResizeHandle(): void {
  const handle = document.getElementById('sidebar-resize-handle');
  const sidebar = document.getElementById('left-sidebar');
  if (!handle || !sidebar) return;

  let isResizing = false;
  let startX = 0;
  let startWidth = 260;

  handle.addEventListener('mousedown', (e) => {
    isResizing = true;
    startX = e.clientX;
    startWidth = sidebar.getBoundingClientRect().width;
    handle.classList.add('resizing');
    document.body.style.cursor = 'ew-resize';
  });

  window.addEventListener('mousemove', (e) => {
    if (!isResizing) return;
    const delta = e.clientX - startX;
    const newWidth = Math.max(200, Math.min(450, startWidth + delta));
    sidebar.style.setProperty('--sidebar-width', `${newWidth}px`);
    sidebar.style.width = `${newWidth}px`;
    sidebar.style.minWidth = `${newWidth}px`;
  });

  window.addEventListener('mouseup', () => {
    if (isResizing) {
      isResizing = false;
      handle.classList.remove('resizing');
      document.body.style.cursor = '';
    }
  });
}

function renderSidebarContent(): void {
  renderProjectsSection();
  renderChatsSection();
  renderWhatsAppSection();
  renderTelegramSection();
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
      '<div style="padding: 6px 8px; font-size: 12px; color: var(--text-muted);">No projects yet</div>';
    return;
  }

  projects.forEach((proj) => {
    const card = document.createElement('div');
    const isCollapsed = sidebarState.isProjectCollapsed(proj.workspace);
    card.className = `project-card ${isCollapsed ? 'collapsed' : ''}`;

    const isProjectActive = sidebarState.getActiveProjectPath() === proj.workspace;
    const hasChildPerm = proj.conversations.some((c) => hasPendingPermission(c.id));

    const header = document.createElement('div');
    header.className = `project-header ${isProjectActive ? 'active' : ''}`;
    header.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chevron-down chevron-icon proj-chevron"></span>
        <span class="ui-icon icon-sidebar-folder"></span>
        <span class="session-title-text" title="${proj.workspace}">${proj.name}</span>
        ${hasChildPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
      </div>
      <div class="project-header-actions">
        <button class="section-action-btn btn-proj-new-chat" title="New Chat in Project">
          <span class="ui-icon icon-sidebar-new-chat"></span>
        </button>
        <button class="section-action-btn btn-proj-delete" title="Delete Project">
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
    });

    header.querySelector('.btn-proj-delete')?.addEventListener('click', async (e) => {
      e.stopPropagation();
      const confirmed = await showConfirmDialog({
        title: 'Delete Project',
        message: `Are you sure you want to remove project "${proj.name}" from your workspace?`,
        confirmText: 'Delete',
        cancelText: 'Cancel',
        isDanger: true,
        icon: 'trash',
      });
      if (confirmed) {
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
        const hasPerm = hasPendingPermission(conv.id);
        item.className = `session-item ${isActive ? 'active' : ''}`;
        item.innerHTML = `
          <div class="session-item-left">
            <span class="session-title-text" title="${conv.title}">${conv.title}</span>
            ${hasPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
          </div>
          <button class="item-more-btn" title="Options">
            <span class="ui-icon icon-sidebar-more-vertical"></span>
          </button>
        `;

        item.addEventListener('click', () => {
          sidebarState.selectSession(conv.id, proj.workspace);
        });

        const moreBtn = item.querySelector<HTMLButtonElement>('.item-more-btn');
        moreBtn?.addEventListener('click', (e) => {
          e.stopPropagation();
          showConversationContextMenu(e, conv, proj.workspace);
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
      '<div style="padding: 6px 8px; font-size: 12px; color: var(--text-muted);">No standalone chats</div>';
    return;
  }

  chats.forEach((chat) => {
    const item = document.createElement('div');
    const isActive = sidebarState.getActiveSessionId() === chat.id;
    const hasPerm = hasPendingPermission(chat.id);
    item.className = `session-item ${isActive ? 'active' : ''}`;
    item.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chats"></span>
        <span class="session-title-text" title="${chat.title}">${chat.title}</span>
        ${hasPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
      </div>
      <button class="item-more-btn" title="Options">
        <span class="ui-icon icon-sidebar-more-vertical"></span>
      </button>
    `;

    item.addEventListener('click', () => {
      sidebarState.selectSession(chat.id, null);
    });

    const moreBtn = item.querySelector<HTMLButtonElement>('.item-more-btn');
    moreBtn?.addEventListener('click', (e) => {
      e.stopPropagation();
      showConversationContextMenu(e, chat, undefined);
    });

    container.appendChild(item);
  });
}

function renderWhatsAppSection(): void {
  const section = document.getElementById('section-whatsapp');
  const container = document.getElementById('whatsapp-items-container');
  const countBadge = document.getElementById('whatsapp-count-badge');
  if (!section || !container) return;

  const contacts = sidebarState.getWhatsAppContacts();
  if (contacts.length === 0) {
    section.style.display = 'none';
    return;
  }

  section.style.display = 'flex';
  if (countBadge) countBadge.textContent = String(contacts.length);
  section.classList.toggle('collapsed', sidebarState.isWhatsAppCollapsed());

  container.innerHTML = '';
  contacts.forEach((contact) => {
    const card = document.createElement('div');
    const isCollapsed = sidebarState.isProjectCollapsed(contact.workspace);
    card.className = `project-card ${isCollapsed ? 'collapsed' : ''}`;

    const isContactActive = sidebarState.getActiveProjectPath() === contact.workspace;
    const hasChildPerm = contact.conversations.some((c) => hasPendingPermission(c.id));

    const header = document.createElement('div');
    header.className = `project-header ${isContactActive ? 'active' : ''}`;
    header.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chevron-down chevron-icon proj-chevron"></span>
        <span class="ui-icon icon-sidebar-user"></span>
        <span class="session-title-text" title="${contact.workspace}">${contact.name}</span>
        ${hasChildPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
      </div>
    `;

    header.addEventListener('click', () => {
      sidebarState.toggleProjectCollapsed(contact.workspace);
    });

    card.appendChild(header);

    if (contact.conversations.length > 0) {
      const convList = document.createElement('div');
      convList.className = 'project-conversations';

      contact.conversations.forEach((conv) => {
        const item = document.createElement('div');
        const isActive = sidebarState.getActiveSessionId() === conv.id;
        const hasPerm = hasPendingPermission(conv.id);
        item.className = `session-item ${isActive ? 'active' : ''}`;
        item.innerHTML = `
          <div class="session-item-left">
            <span class="session-title-text" title="${conv.title}">${conv.title}</span>
            ${hasPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
          </div>
          <button class="item-delete-btn" title="Delete conversation">
            <span class="ui-icon icon-sidebar-trash"></span>
          </button>
        `;

        item.addEventListener('click', () => {
          sidebarState.selectSession(conv.id, contact.workspace);
        });

        // Direct delete action for channel session with confirmation prompt
        const deleteBtn = item.querySelector<HTMLButtonElement>('.item-delete-btn');
        deleteBtn?.addEventListener('click', async (e) => {
          e.stopPropagation();
          const confirmed = await showConfirmDialog({
            title: 'Delete Conversation',
            message: `Are you sure you want to delete conversation "${conv.title}"?`,
            confirmText: 'Delete',
            cancelText: 'Cancel',
            isDanger: true,
            icon: 'trash',
          });
          if (confirmed) {
            await deleteSessionIpc(conv.id);
            if (sidebarState.getActiveSessionId() === conv.id) {
              sidebarState.selectSession(null, null);
            }
            await refreshSidebarContent();
          }
        });

        convList.appendChild(item);
      });

      card.appendChild(convList);
    }

    container.appendChild(card);
  });
}

function renderTelegramSection(): void {
  const section = document.getElementById('section-telegram');
  const container = document.getElementById('telegram-items-container');
  const countBadge = document.getElementById('telegram-count-badge');
  if (!section || !container) return;

  const contacts = sidebarState.getTelegramContacts();
  if (contacts.length === 0) {
    section.style.display = 'none';
    return;
  }

  section.style.display = 'flex';
  if (countBadge) countBadge.textContent = String(contacts.length);
  section.classList.toggle('collapsed', sidebarState.isTelegramCollapsed());

  container.innerHTML = '';
  contacts.forEach((contact) => {
    const card = document.createElement('div');
    const isCollapsed = sidebarState.isProjectCollapsed(contact.workspace);
    card.className = `project-card ${isCollapsed ? 'collapsed' : ''}`;

    const isContactActive = sidebarState.getActiveProjectPath() === contact.workspace;
    const hasChildPerm = contact.conversations.some((c) => hasPendingPermission(c.id));

    const header = document.createElement('div');
    header.className = `project-header ${isContactActive ? 'active' : ''}`;
    header.innerHTML = `
      <div class="session-item-left">
        <span class="ui-icon icon-sidebar-chevron-down chevron-icon proj-chevron"></span>
        <span class="ui-icon icon-sidebar-user"></span>
        <span class="session-title-text" title="${contact.workspace}">${contact.name}</span>
        ${hasChildPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
      </div>
    `;

    header.addEventListener('click', () => {
      sidebarState.toggleProjectCollapsed(contact.workspace);
    });

    card.appendChild(header);

    if (contact.conversations.length > 0) {
      const convList = document.createElement('div');
      convList.className = 'project-conversations';

      contact.conversations.forEach((conv) => {
        const item = document.createElement('div');
        const isActive = sidebarState.getActiveSessionId() === conv.id;
        const hasPerm = hasPendingPermission(conv.id);
        item.className = `session-item ${isActive ? 'active' : ''}`;
        item.innerHTML = `
          <div class="session-item-left">
            <span class="session-title-text" title="${conv.title}">${conv.title}</span>
            ${hasPerm ? '<span class="session-pending-perm-dot" title="Requires permission approval"></span>' : ''}
          </div>
          <button class="item-delete-btn" title="Delete conversation">
            <span class="ui-icon icon-sidebar-trash"></span>
          </button>
        `;

        item.addEventListener('click', () => {
          sidebarState.selectSession(conv.id, contact.workspace);
        });

        // Direct delete action for channel session with confirmation prompt
        const deleteBtn = item.querySelector<HTMLButtonElement>('.item-delete-btn');
        deleteBtn?.addEventListener('click', async (e) => {
          e.stopPropagation();
          const confirmed = await showConfirmDialog({
            title: 'Delete Conversation',
            message: `Are you sure you want to delete conversation "${conv.title}"?`,
            confirmText: 'Delete',
            cancelText: 'Cancel',
            isDanger: true,
            icon: 'trash',
          });
          if (confirmed) {
            await deleteSessionIpc(conv.id);
            if (sidebarState.getActiveSessionId() === conv.id) {
              sidebarState.selectSession(null, null);
            }
            await refreshSidebarContent();
          }
        });

        convList.appendChild(item);
      });

      card.appendChild(convList);
    }

    container.appendChild(card);
  });
}

/**
 * Renders and positions the floating context menu for a conversation item.
 */
function showConversationContextMenu(
  e: MouseEvent,
  conv: SidebarConversation,
  projectWorkspace?: string
): void {
  dismissContextMenu();

  const target = e.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();

  const menu = document.createElement('div');
  menu.className = 'session-context-menu';

  // Position adjacent to the trigger button
  const top = Math.min(window.innerHeight - 220, rect.bottom + 2);
  const left = Math.min(window.innerWidth - 160, rect.left);
  menu.style.top = `${top}px`;
  menu.style.left = `${left}px`;

  menu.innerHTML = `
    <button class="context-menu-item" id="ctx-share">
      <span class="ui-icon icon-sidebar-share"></span>
      <span>Share</span>
    </button>
    <button class="context-menu-item" id="ctx-rename">
      <span class="ui-icon icon-sidebar-pencil"></span>
      <span>Rename</span>
    </button>
    <button class="context-menu-item" id="ctx-move">
      <span class="ui-icon icon-sidebar-folder-input"></span>
      <span>Move to...</span>
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

  // 1. Share action
  menu.querySelector('#ctx-share')?.addEventListener('click', async (evt) => {
    evt.stopPropagation();
    dismissContextMenu();
    try {
      await navigator.clipboard.writeText(conv.id);
      console.debug('[Sidebar] Copied session ID to clipboard:', conv.id);
    } catch {
      // Fallback
    }
  });

  // 2. Rename action
  menu.querySelector('#ctx-rename')?.addEventListener('click', async (evt) => {
    evt.stopPropagation();
    dismissContextMenu();
    const newTitle = await showPromptDialog({
      title: 'Rename Conversation',
      message: 'Enter a new title for this conversation:',
      defaultValue: conv.title,
      placeholder: 'Conversation title',
      confirmText: 'Ok',
      cancelText: 'Cancel',
      icon: 'pencil',
    });
    if (newTitle && newTitle.trim().length > 0) {
      await renameSessionIpc(conv.id, newTitle.trim());
      await refreshSidebarContent();
    }
  });

  // 3. Move to... action
  menu.querySelector('#ctx-move')?.addEventListener('click', async (evt) => {
    evt.stopPropagation();
    dismissContextMenu();

    const projects = sidebarState.getProjects();
    const options = ['[Standalone Chat]', ...projects.map((p) => p.name)];
    const choice = await showPromptDialog({
      title: 'Move Conversation',
      message: `Move conversation to:\n${options.map((opt, idx) => `${idx + 1}. ${opt}`).join('\n')}`,
      defaultValue: '1',
      placeholder: 'Enter choice number',
      confirmText: 'Ok',
      cancelText: 'Cancel',
    });

    if (choice) {
      const idx = parseInt(choice.trim(), 10) - 1;
      if (idx === 0) {
        // Move to standalone
        await moveSessionIpc(conv.id, '');
        if (sidebarState.getActiveSessionId() === conv.id) {
          sidebarState.setActiveProjectPath(null);
        }
        await refreshSidebarContent();
      } else if (idx > 0 && idx < options.length) {
        const targetProj = projects[idx - 1];
        await moveSessionIpc(conv.id, targetProj.workspace);
        if (sidebarState.getActiveSessionId() === conv.id) {
          sidebarState.setActiveProjectPath(targetProj.workspace);
        }
        await refreshSidebarContent();
      }
    }
  });

  // 4. Fork action
  menu.querySelector('#ctx-fork')?.addEventListener('click', async (evt) => {
    evt.stopPropagation();
    dismissContextMenu();
    const newId = await forkSessionIpc(conv.id);
    sidebarState.selectSession(newId, projectWorkspace || null);
    await refreshSidebarContent();
    console.debug('[Sidebar] Forked conversation:', newId);
  });

  // 5. Delete action
  menu.querySelector('#ctx-delete')?.addEventListener('click', async (evt) => {
    evt.stopPropagation();
    dismissContextMenu();
    const confirmed = await showConfirmDialog({
      title: 'Delete Conversation',
      message: `Are you sure you want to delete conversation "${conv.title}"?`,
      confirmText: 'Delete',
      cancelText: 'Cancel',
      isDanger: true,
      icon: 'trash',
    });
    if (confirmed) {
      await deleteSessionIpc(conv.id);
      if (sidebarState.getActiveSessionId() === conv.id) {
        sidebarState.selectSession(null, null);
      }
      await refreshSidebarContent();
    }
  });

  document.body.appendChild(menu);
  activeContextMenu = menu;
}
