// Main Content Topbar Controller & DOM Coordinator

import { renameSessionIpc } from '../../left-sidebar/ipc.js';
import { refreshSidebarContent } from '../../left-sidebar/sidebar.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { rightSidebarState } from '../../right-sidebar/state.js';
import { refreshTodoPanel } from '../../right-sidebar/todo-panel/todo-panel.js';
import { showPromptDialog } from '../../shared/dialog.js';
import { getTopbarInfoIpc } from './ipc.js';
import { topbarState } from './state.js';

export function initTopbar(): void {
  setupButtons();
  setupInlineTitleRename();

  // Re-fetch when sidebar active session or project changes
  sidebarState.subscribe(async () => {
    await refreshTopbar();
  });

  // Re-render when topbar state changes
  topbarState.subscribe(() => {
    renderTopbar();
  });

  // Re-render when right sidebar state changes
  rightSidebarState.subscribe(() => {
    renderTopbar();
  });

  // Initial load
  refreshTopbar();
}

export async function refreshTopbar(): Promise<void> {
  const sessionId = sidebarState.getActiveSessionId() || undefined;
  const workspacePath = sidebarState.getActiveProjectPath() || undefined;

  const data = await getTopbarInfoIpc(sessionId, workspacePath);

  topbarState.setTitle(data.title);
  topbarState.setProjectContext(data.is_project, data.project_name || null);
  topbarState.setTodoCounts(
    data.unfinished_todo_count || 0,
    data.total_todo_count || 0
  );

  if (data.git_stats) {
    topbarState.setGitStats(data.git_stats);
  } else {
    topbarState.setGitStats({
      insertions: 0,
      deletions: 0,
      files_changed: 0,
      is_git_repo: false,
    });
  }
}

function setupButtons(): void {
  // Todo button (opens/switches to session tasks panel)
  document.getElementById('btn-topbar-todo')?.addEventListener('click', async () => {
    rightSidebarState.openPanel('todos');
    await refreshTodoPanel();
  });

  // Terminal button
  document.getElementById('btn-topbar-terminal')?.addEventListener('click', () => {
    const isOpen = topbarState.toggleTerminal();
    console.debug('[Topbar] Terminal toggled:', isOpen);
  });
}

function setupInlineTitleRename(): void {
  const titleEl = document.getElementById('topbar-session-title');
  if (!titleEl) return;

  titleEl.addEventListener('dblclick', async () => {
    const activeSessionId = sidebarState.getActiveSessionId();
    if (!activeSessionId) return;

    const currentTitle = topbarState.getTitle();
    const newTitle = await showPromptDialog({
      title: 'Rename Conversation',
      message: 'Enter a new title for this conversation:',
      defaultValue: currentTitle,
      placeholder: 'Conversation title',
      confirmText: 'Ok',
      cancelText: 'Cancel',
      icon: 'pencil',
    });

    if (newTitle && newTitle.trim().length > 0 && newTitle.trim() !== currentTitle) {
      await renameSessionIpc(activeSessionId, newTitle.trim());
      topbarState.setTitle(newTitle.trim());
      await refreshSidebarContent();
    }
  });
}

function renderTopbar(): void {
  // 1. Session Title
  const titleEl = document.getElementById('topbar-session-title');
  if (titleEl) {
    titleEl.textContent = topbarState.getTitle();
    titleEl.title = topbarState.getTitle();
  }

  // 2. Project badge
  const badgeEl = document.getElementById('topbar-project-badge');
  if (badgeEl) {
    const isProject = topbarState.getIsProject();
    const projName = topbarState.getProjectName();
    badgeEl.classList.toggle('visible', isProject && !!projName);
    if (projName) {
      badgeEl.textContent = projName;
      badgeEl.title = `Project: ${projName}`;
    }
  }

  // 3. Todo topbar button (Visible whenever session has any tasks present)
  const todoBtn = document.getElementById('btn-topbar-todo');
  const totalCount = topbarState.getTotalTodoCount();
  const unfinishedCount = topbarState.getUnfinishedTodoCount();

  if (todoBtn) {
    if (totalCount > 0) {
      todoBtn.style.display = 'flex';
      if (unfinishedCount > 0) {
        todoBtn.title = `Session Tasks (${unfinishedCount} pending, ${totalCount} total)`;
      } else {
        todoBtn.title = `Session Tasks (All ${totalCount} completed)`;
      }
    } else {
      todoBtn.style.display = 'none';
    }

    const isTodosOpen =
      rightSidebarState.getIsOpen() &&
      rightSidebarState.getActivePanel() === 'todos';
    todoBtn.classList.toggle('active', isTodosOpen);
  }

  // 4. Git stats badges and toggle button
  const gitStatsWrapper = document.getElementById('topbar-git-stats');
  const insertionsEl = document.getElementById('git-stat-insertions');
  const deletionsEl = document.getElementById('git-stat-deletions');
  const gitBtn = document.getElementById('btn-topbar-git-commit');

  if (gitStatsWrapper && insertionsEl && deletionsEl && gitBtn) {
    const stats = topbarState.getGitStats();
    const hasChanges = stats.is_git_repo && (stats.insertions > 0 || stats.deletions > 0);

    gitStatsWrapper.classList.toggle('visible', hasChanges);

    if (stats.insertions > 0) {
      insertionsEl.textContent = `+${stats.insertions}`;
      insertionsEl.style.display = 'inline';
    } else {
      insertionsEl.style.display = 'none';
    }

    if (stats.deletions > 0) {
      deletionsEl.textContent = `-${stats.deletions}`;
      deletionsEl.style.display = 'inline';
    } else {
      deletionsEl.style.display = 'none';
    }

    const isGitOpen =
      rightSidebarState.getIsOpen() &&
      rightSidebarState.getActivePanel() === 'git';
    gitBtn.classList.toggle('active', isGitOpen);
  }

  // 5. Terminal button active state
  const terminalBtn = document.getElementById('btn-topbar-terminal');
  terminalBtn?.classList.toggle('active', topbarState.getIsTerminalOpen());
}
