// ============================================================================
// Session Tasks / Todo Panel Controller & DOM Renderer
//
// Hey friend! This is the main controller and DOM renderer for our Session
// Tasks right sidebar panel. It renders a clean, stylish task manager where
// users can track the agent's progress, toggle task completion, add quick
// tasks, and filter by status.
// ============================================================================

import { sidebarState } from '../../left-sidebar/state.js';
import { refreshTopbar } from '../../main-content/topbar/topbar.js';
import { rightSidebarState } from '../state.js';
import {
  createSessionTodoIpc,
  deleteSessionTodoIpc,
  getSessionTodosIpc,
  updateSessionTodoStatusIpc,
} from './ipc.js';
import { todoPanelState } from './state.js';
import type { TodoFilter, TodoItemDto } from './types.js';

/**
 * Fetches fresh todo items from the backend for the currently active session.
 */
export async function refreshTodoPanel(sessionId?: string): Promise<void> {
  const activeId = sessionId || sidebarState.getActiveSessionId() || undefined;
  if (!activeId) {
    todoPanelState.setTodos([]);
    return;
  }

  try {
    todoPanelState.setIsLoading(true);
    const todos = await getSessionTodosIpc(activeId);
    todoPanelState.setTodos(todos);
  } catch (err) {
    console.warn('[TodoPanel] Failed to fetch session todos:', err);
  } finally {
    todoPanelState.setIsLoading(false);
  }
}

/**
 * Main DOM rendering function for the Todo Panel.
 * Mounts the complete UI structure into the provided aside/container element.
 */
export function renderTodoPanel(aside: HTMLElement): void {
  // 1. Create left drag resize handle
  const resizeHandle = document.createElement('div');
  resizeHandle.className = 'right-sidebar-resize-handle';
  setupPanelResizeHandle(resizeHandle);
  aside.appendChild(resizeHandle);

  // 2. Main panel container
  const container = document.createElement('div');
  container.className = 'todo-panel-container';

  // 3. Top Header Bar
  const header = createHeader();
  container.appendChild(header);

  // 4. Filter Toolbar Chips
  const filtersBar = createFiltersBar();
  container.appendChild(filtersBar);

  // 5. Scrollable Todo Items List
  const listContainer = createTodoList();
  container.appendChild(listContainer);

  // 6. Quick Add Footer
  const footer = createQuickAddFooter();
  container.appendChild(footer);

  aside.appendChild(container);
}

/**
 * Builds the top header with session title, unfinished task count, and close action.
 */
function createHeader(): HTMLElement {
  const header = document.createElement('div');
  header.className = 'todo-panel-header';

  const left = document.createElement('div');
  left.className = 'todo-panel-header-left';

  const title = document.createElement('span');
  title.className = 'todo-panel-title';
  title.textContent = 'Session Tasks';
  left.appendChild(title);

  const unfinishedCount = todoPanelState.getUnfinishedCount();
  const totalCount = todoPanelState.getTodos().length;

  if (totalCount > 0) {
    const badge = document.createElement('span');
    badge.className = `todo-header-badge ${unfinishedCount === 0 ? 'all-done' : ''}`;
    badge.textContent = unfinishedCount === 0 ? `All done (${totalCount})` : `${unfinishedCount} pending`;
    left.appendChild(badge);
  }

  header.appendChild(left);

  // Close Action Button
  const actions = document.createElement('div');
  actions.className = 'todo-panel-header-actions';

  const closeBtn = document.createElement('button');
  closeBtn.className = 'todo-panel-icon-btn';
  closeBtn.title = 'Close Panel';
  closeBtn.innerHTML = '<span class="ui-icon icon-todo-close"></span>';
  closeBtn.addEventListener('click', () => {
    rightSidebarState.setIsOpen(false);
  });
  actions.appendChild(closeBtn);

  header.appendChild(actions);
  return header;
}

/**
 * Builds the filter chips toolbar ('All', 'Pending', 'Completed').
 */
function createFiltersBar(): HTMLElement {
  const bar = document.createElement('div');
  bar.className = 'todo-filters-bar';

  const currentFilter = todoPanelState.getFilter();
  const allTodos = todoPanelState.getTodos();
  const pendingCount = todoPanelState.getUnfinishedCount();
  const completedCount = todoPanelState.getCompletedCount();

  const filters: { id: TodoFilter; label: string; count: number }[] = [
    { id: 'all', label: 'All', count: allTodos.length },
    { id: 'pending', label: 'Pending', count: pendingCount },
    { id: 'completed', label: 'Completed', count: completedCount },
  ];

  filters.forEach(({ id, label, count }) => {
    const chip = document.createElement('button');
    chip.className = `todo-filter-chip ${currentFilter === id ? 'active' : ''}`;
    chip.textContent = `${label} (${count})`;
    chip.addEventListener('click', () => {
      todoPanelState.setFilter(id);
      rightSidebarState.notify();
    });
    bar.appendChild(chip);
  });

  return bar;
}

/**
 * Builds the scrollable list of todo item cards or empty state.
 */
function createTodoList(): HTMLElement {
  const scrollList = document.createElement('div');
  scrollList.className = 'todo-list-scroll';

  const filteredTodos = todoPanelState.getFilteredTodos();

  if (filteredTodos.length === 0) {
    const emptyState = document.createElement('div');
    emptyState.className = 'todo-empty-state';

    const icon = document.createElement('div');
    icon.className = 'todo-empty-icon';
    emptyState.appendChild(icon);

    const title = document.createElement('div');
    title.className = 'todo-empty-title';
    title.textContent = 'No tasks found';
    emptyState.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'todo-empty-desc';
    desc.textContent =
      todoPanelState.getTodos().length === 0
        ? 'The agent will automatically create session tasks here when working.'
        : 'No tasks match the selected filter.';
    emptyState.appendChild(desc);

    scrollList.appendChild(emptyState);
    return scrollList;
  }

  filteredTodos.forEach((item) => {
    const card = createTodoCard(item);
    scrollList.appendChild(card);
  });

  return scrollList;
}

/**
 * Creates a single interactive Todo Card.
 */
function createTodoCard(item: TodoItemDto): HTMLElement {
  const card = document.createElement('div');
  card.className = `todo-card ${item.status}`;
  card.dataset.id = item.id;

  // 1. Checkbox Toggle Button
  const checkBtn = document.createElement('button');
  checkBtn.className = 'todo-checkbox-btn';
  checkBtn.title =
    item.status === 'completed' ? 'Mark as pending' : 'Mark as completed';

  checkBtn.innerHTML = `
    <span class="todo-checkbox-check-icon"></span>
    <span class="todo-checkbox-dot-icon"></span>
  `;

  checkBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const activeSessionId = sidebarState.getActiveSessionId();
    if (!activeSessionId) return;

    const nextStatus = item.status === 'completed' ? 'pending' : 'completed';
    try {
      const updated = await updateSessionTodoStatusIpc(
        activeSessionId,
        item.id,
        nextStatus
      );
      todoPanelState.setTodos(updated);
      await refreshTopbar();
      rightSidebarState.notify();
    } catch (err) {
      console.error('[TodoPanel] Failed to update status:', err);
    }
  });
  card.appendChild(checkBtn);

  // 2. Body: Text description + Meta Tags
  const body = document.createElement('div');
  body.className = 'todo-card-body';

  const text = document.createElement('div');
  text.className = 'todo-card-text';
  text.textContent = item.content;
  body.appendChild(text);

  const meta = document.createElement('div');
  meta.className = 'todo-card-meta';

  const idPill = document.createElement('span');
  idPill.className = 'todo-id-pill';
  idPill.textContent = `#${item.id}`;
  meta.appendChild(idPill);

  const statusBadge = document.createElement('span');
  statusBadge.className = `todo-status-badge ${item.status}`;
  statusBadge.textContent =
    item.status === 'in_progress' ? 'In Progress' : item.status;
  meta.appendChild(statusBadge);

  const priorityBadge = document.createElement('span');
  priorityBadge.className = `todo-priority-badge ${item.priority}`;
  priorityBadge.textContent = item.priority;
  meta.appendChild(priorityBadge);

  body.appendChild(meta);
  card.appendChild(body);

  // 3. Hover Actions (Delete)
  const actions = document.createElement('div');
  actions.className = 'todo-card-actions';

  const deleteBtn = document.createElement('button');
  deleteBtn.className = 'todo-delete-btn';
  deleteBtn.title = 'Delete Task';
  deleteBtn.innerHTML = '<span class="ui-icon icon-todo-trash"></span>';

  deleteBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const activeSessionId = sidebarState.getActiveSessionId();
    if (!activeSessionId) return;

    try {
      const updated = await deleteSessionTodoIpc(activeSessionId, item.id);
      todoPanelState.setTodos(updated);
      await refreshTopbar();
      rightSidebarState.notify();
    } catch (err) {
      console.error('[TodoPanel] Failed to delete task:', err);
    }
  });

  actions.appendChild(deleteBtn);
  card.appendChild(actions);

  return card;
}

/**
 * Creates the quick-add footer input bar.
 */
function createQuickAddFooter(): HTMLElement {
  const footer = document.createElement('div');
  footer.className = 'todo-add-footer';

  const form = document.createElement('form');
  form.className = 'todo-add-form';

  const input = document.createElement('input');
  input.className = 'todo-add-input';
  input.type = 'text';
  input.placeholder = 'Add a new task...';
  input.autocomplete = 'off';

  const submitBtn = document.createElement('button');
  submitBtn.className = 'todo-add-submit-btn';
  submitBtn.type = 'submit';
  submitBtn.title = 'Add Task';
  submitBtn.innerHTML = '<span class="ui-icon icon-todo-add"></span>';

  form.appendChild(input);
  form.appendChild(submitBtn);

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    const content = input.value.trim();
    if (!content) return;

    const activeSessionId = sidebarState.getActiveSessionId();
    if (!activeSessionId) return;

    try {
      const updated = await createSessionTodoIpc(activeSessionId, content);
      todoPanelState.setTodos(updated);
      input.value = '';
      await refreshTopbar();
      rightSidebarState.notify();
    } catch (err) {
      console.error('[TodoPanel] Failed to create task:', err);
    }
  });

  footer.appendChild(form);
  return footer;
}

/**
 * Attaches mouse drag listeners to the left resize handle for smoothly resizing panel width.
 */
function setupPanelResizeHandle(handle: HTMLElement): void {
  let isDragging = false;
  let startX = 0;
  let startWidth = 0;

  const onMouseMove = (e: MouseEvent) => {
    if (!isDragging) return;
    const deltaX = startX - e.clientX;
    const newWidth = startWidth + deltaX;
    rightSidebarState.setWidth(newWidth);
  };

  const onMouseUp = () => {
    if (!isDragging) return;
    isDragging = false;
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', onMouseUp);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  };

  handle.addEventListener('mousedown', (e) => {
    isDragging = true;
    startX = e.clientX;
    startWidth = rightSidebarState.getWidth();
    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
    document.body.style.cursor = 'ew-resize';
    document.body.style.userSelect = 'none';
    e.preventDefault();
  });
}
