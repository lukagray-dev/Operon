// ============================================================================
// Main Content Topbar Controller & DOM Coordinator for VS Code
// ============================================================================

import { toggleSidebar } from '../../left-sidebar/sidebar.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { todoPanelState } from '../../right-sidebar/state.js';
import { refreshTodoPanel } from '../../right-sidebar/todo-panel.js';
import { getTopbarInfoIpc } from './ipc.js';
import { topbarState } from './state.js';

let sessionTitleEl: HTMLElement | null = null;
let projectBadgeEl: HTMLElement | null = null;
let btnToggleTodosEl: HTMLElement | null = null;

/**
 * Initializes topbar DOM hooks, button event listeners, and reactive subscriptions.
 */
export function initTopbar(): void {
  const btnToggleSidebar = document.getElementById('btn-toggle-sidebar');
  sessionTitleEl = document.getElementById('topbar-session-title');
  projectBadgeEl = document.getElementById('topbar-project-badge');
  btnToggleTodosEl = document.getElementById('btn-toggle-todos');

  btnToggleSidebar?.addEventListener('click', () => {
    toggleSidebar();
  });

  btnToggleTodosEl?.addEventListener('click', async () => {
    todoPanelState.toggle();
    if (todoPanelState.getIsOpen()) {
      await refreshTodoPanel();
    }
  });

  // Re-fetch topbar metadata when sidebar active session changes
  sidebarState.subscribe(async () => {
    await refreshTopbar();
  });

  // Re-render when topbar state updates
  topbarState.subscribe(() => {
    renderTopbar();
  });

  // Re-render when right sidebar state updates
  todoPanelState.subscribe(() => {
    renderTopbar();
  });

  // Initial load
  refreshTopbar();
  console.log('[Operon Topbar] Initialized.');
}

/**
 * Fetches fresh topbar status from the backend for active session & project.
 */
export async function refreshTopbar(): Promise<void> {
  const sessionId = sidebarState.getActiveSessionId() || undefined;
  const workspacePath = sidebarState.getActiveProjectPath() || undefined;

  try {
    const data = await getTopbarInfoIpc(sessionId, workspacePath);
    topbarState.setTitle(data.title);
    topbarState.setProjectContext(data.is_project, data.project_name || null);
    topbarState.setTodoCounts(
      data.unfinished_todo_count || 0,
      data.total_todo_count || 0
    );
  } catch (err) {
    console.warn('[Topbar] Failed to load topbar info:', err);
  }
}

/**
 * Updates topbar DOM elements to reflect current topbar state.
 */
function renderTopbar(): void {
  if (sessionTitleEl) {
    sessionTitleEl.textContent = topbarState.getTitle();
    sessionTitleEl.title = topbarState.getTitle();
  }

  if (projectBadgeEl) {
    const projectName = topbarState.getProjectName();
    if (projectName) {
      projectBadgeEl.textContent = projectName;
      projectBadgeEl.title = `Project: ${projectName}`;
      projectBadgeEl.classList.add('visible');
    } else {
      projectBadgeEl.textContent = '';
      projectBadgeEl.classList.remove('visible');
    }
  }

  // Todo topbar button: Only visible when active session has tasks present (matching GUI)
  if (btnToggleTodosEl) {
    const { unfinished, total } = topbarState.getTodoCounts();
    if (total > 0) {
      btnToggleTodosEl.style.display = 'flex';
      if (unfinished > 0) {
        btnToggleTodosEl.title = `Session Tasks (${unfinished} pending, ${total} total)`;
      } else {
        btnToggleTodosEl.title = `Session Tasks (All ${total} completed)`;
      }
    } else {
      btnToggleTodosEl.style.display = 'none';
    }

    const badge = btnToggleTodosEl.querySelector('.todo-counter-badge');
    if (badge) {
      if (total > 0) {
        badge.textContent = unfinished > 0 ? `${unfinished}` : '✓';
        badge.classList.remove('hidden');
      } else {
        badge.classList.add('hidden');
      }
    }

    btnToggleTodosEl.classList.toggle('active', todoPanelState.getIsOpen());
  }
}
