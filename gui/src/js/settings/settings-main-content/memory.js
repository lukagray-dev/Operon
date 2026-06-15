'use strict';

/**
 * memory.js
 *
 * Memories settings page for the Operon settings panel.
 *
 * Manages the memory listing view inside the [data-memory-host] container.
 * Responsibilities:
 *  1. Query SQLite memory database through Tauri IPC wrapper (getMemories).
 *  2. Display memory cards in a list, ordered by update timestamp.
 *  3. Support real-time search filtering on memory content.
 *  4. Quick add memory row directly inside the page.
 *  5. Inline editing with change detection (highlights "Save" button when modifications exist).
 *  6. Deletion with instant UI synchronization.
 */

import { showError, showSuccess } from '../../shared/toast.js';
import {
  activeCategory,
  renderInlineStatus,
  escapeHtml,
  normalizeErrorMessage,
} from '../settings-panel.js';
import * as IPC from '../../shared/ipc.js';

// ── Transient Module State ────────────────────────────────────────────────────

/**
 * local state for the Memories page.
 * Cleared on settings dialog closure via resetMemorySettingsState().
 */
const memoriesState = {
  /** @type {Array<Object>} List of all memories retrieved from SQLite */
  memories: [],
  /** @type {string} Current query typed in the search box */
  searchQuery: '',
  /** Flag for async listing operation */
  loading: false,
  /** ID of memory card currently saving to prevent duplicate clicks */
  savingId: null,
  /** Flag for add memory operation */
  adding: false,
};

// ── State Reset ───────────────────────────────────────────────────────────────

/**
 * Resets the transient state of the Memories tab.
 * Called by settings-panel.js when the dialog is closed.
 */
function resetMemorySettingsState() {
  memoriesState.memories = [];
  memoriesState.searchQuery = '';
  memoriesState.loading = false;
  memoriesState.savingId = null;
  memoriesState.adding = false;
}

// ── Hydration Entry Point ─────────────────────────────────────────────────────

/**
 * Hydrates the Memories settings page.
 * Called by settings-panel.js after the category page scaffold is injected.
 * Loads the memories database if not already cached and renders the interface.
 *
 * @param {HTMLElement} modal - The root settings dialog element.
 */
async function hydrateMemoryPage(modal) {
  // Guard: don't perform hydration if navigation changed in the meantime
  if (!modal || activeCategory !== 'memory') return;

  // Initial loading trigger
  if (memoriesState.memories.length === 0 && !memoriesState.loading) {
    await fetchMemoriesFromDb(modal);
  }

  // Draw the initial state
  renderMemoryStage(modal);
}

// ── Database Interaction ──────────────────────────────────────────────────────

/**
 * Fetches all memories from the database and updates the transient list state.
 *
 * @param {HTMLElement} modal - Dialog root element.
 */
async function fetchMemoriesFromDb(modal) {
  memoriesState.loading = true;
  renderMemoryStage(modal);

  try {
    // Invoke Tauri backend command
    const list = await IPC.getMemories();
    memoriesState.memories = Array.isArray(list) ? list : [];
  } catch (error) {
    memoriesState.memories = [];
    showError(normalizeErrorMessage(error, 'Failed to load memories from database.'));
  } finally {
    memoriesState.loading = false;
    renderMemoryStage(modal);
  }
}

// ── Stage Renderer ────────────────────────────────────────────────────────────

/**
 * Re-renders the content of [data-memory-host] container based on current memoriesState.
 *
 * @param {HTMLElement} modal - Dialog root element.
 */
function renderMemoryStage(modal) {
  const host = modal?.querySelector('[data-memory-host]');
  if (!host) return;

  // 1. Build the control toolbar (search and reload)
  const toolbarHtml = `
    <div class="settings-memory__toolbar">
      <div class="settings-memory__search-container">
        <img src="./assets/icons/settings/search.svg" class="settings-memory__search-icon" alt="" width="14" height="14" draggable="false">
        <input type="text"
               class="settings-input settings-memory__search-input"
               placeholder="Search memories..."
               value="${escapeHtml(memoriesState.searchQuery)}"
               data-memory-search-input>
        <button class="settings-memory__clear-btn"
                type="button"
                data-memory-search-clear
                style="display: ${memoriesState.searchQuery ? 'flex' : 'none'};"
                title="Clear search">
          <img src="./assets/icons/settings/close.svg" alt="Clear" width="10" height="10" draggable="false">
        </button>
      </div>
      <button class="btn btn--ghost btn--sm settings-memory__refresh-btn"
              type="button"
              data-memory-refresh
              title="Reload memories"
              ${memoriesState.loading ? 'disabled' : ''}>
        <img src="./assets/icons/settings/refresh.svg" alt="Reload" width="14" height="14" draggable="false">
      </button>
    </div>
  `;

  // 2. Build the Quick Add Memory container (input and button side-by-side)
  const addCardHtml = `
    <div class="settings-memory__add-container">
      <textarea class="settings-memory__textarea settings-memory__textarea--new"
                placeholder="Type a new memory to add..."
                rows="1"
                data-memory-add-input
                ${memoriesState.adding ? 'disabled' : ''}></textarea>
      <button class="btn btn--primary btn--sm settings-memory__add-btn"
              type="button"
              data-memory-add-btn
              ${memoriesState.adding ? 'disabled' : ''}
              title="Add memory">
        ${memoriesState.adding 
          ? '<span class="model-selector__spinner" aria-hidden="true"></span>' 
          : '<img src="./assets/icons/settings/plus.svg" alt="" width="12" height="12" draggable="false">'}
      </button>
    </div>
  `;

  // 3. Render state status if loading
  if (memoriesState.loading) {
    host.innerHTML = `
      <div class="settings-memory__container">
        ${toolbarHtml}
        <div class="settings-memory__list-container">
          ${renderInlineStatus('Loading memories...', true)}
        </div>
        ${addCardHtml}
      </div>
    `;
    bindMemoryPageEvents(modal);
    return;
  }

  // 4. Filter memories list locally using query
  const query = memoriesState.searchQuery.toLowerCase().trim();
  const filtered = memoriesState.memories.filter(item => 
    String(item.content || '').toLowerCase().includes(query)
  );

  // 5. Generate list items HTML
  let listHtml = '';
  if (filtered.length === 0) {
    const emptyMsg = memoriesState.searchQuery 
      ? `No memories found matching "${memoriesState.searchQuery}".`
      : 'No memories stored yet.';
    listHtml = `<div class="settings-memory__empty-state">${renderInlineStatus(emptyMsg)}</div>`;
  } else {
    listHtml = filtered.map(item => {
      const isSaving = memoriesState.savingId === item.id;
      return `
        <div class="settings-memory__item-card" data-memory-card-id="${item.id}">
          <div class="settings-memory__item-header">
            <div class="settings-memory__item-meta">
              <span class="settings-memory__item-id">ID: ${item.id}</span>
              <span class="settings-memory__item-time">${formatDate(item.updatedAt || item.createdAt)}</span>
            </div>
            <div class="settings-memory__item-actions">
              <button class="btn btn--ghost btn--sm settings-memory__action-save"
                      type="button"
                      data-memory-save-btn="${item.id}"
                      title="Save changes"
                      disabled>
                ${isSaving ? '<span class="model-selector__spinner" aria-hidden="true"></span>' : 'Save'}
              </button>
              <button class="btn btn--ghost btn--sm settings-memory__action-delete"
                      type="button"
                      data-memory-delete-btn="${item.id}"
                      title="Delete memory">
                <img src="./assets/icons/settings/delete.svg" alt="Delete" width="14" height="14" draggable="false">
              </button>
            </div>
          </div>
          <div class="settings-memory__item-body">
            <textarea class="settings-memory__textarea"
                      data-memory-content-textarea="${item.id}"
                      rows="1"
                      ${isSaving ? 'disabled' : ''}>${escapeHtml(item.content)}</textarea>
          </div>
        </div>
      `;
    }).join('');
  }

  // Inject everything into the host element
  host.innerHTML = `
    <div class="settings-memory__container">
      ${toolbarHtml}
      <div class="settings-memory__list-container">
        <div class="settings-memory__list">
          ${listHtml}
        </div>
      </div>
      ${addCardHtml}
    </div>
  `;

  // Bind interaction event listeners
  bindMemoryPageEvents(modal);
}

// ── Event Bindings ────────────────────────────────────────────────────────────

/**
 * Binds DOM action handlers inside the Memories setting workspace.
 *
 * @param {HTMLElement} modal - The dialog wrapper element.
 */
function bindMemoryPageEvents(modal) {
  const host = modal?.querySelector('[data-memory-host]');
  if (!host) return;

  // Search input change detection
  const searchInput = host.querySelector('[data-memory-search-input]');
  if (searchInput instanceof HTMLInputElement) {
    searchInput.addEventListener('input', () => {
      memoriesState.searchQuery = searchInput.value;
      const clearBtn = host.querySelector('[data-memory-search-clear]');
      if (clearBtn instanceof HTMLElement) {
        clearBtn.style.display = searchInput.value ? 'flex' : 'none';
      }
      // Re-render matching cards list dynamically
      renderMemoryStage(modal);
    });
  }

  // Clear search query button
  const clearBtn = host.querySelector('[data-memory-search-clear]');
  clearBtn?.addEventListener('click', () => {
    memoriesState.searchQuery = '';
    renderMemoryStage(modal);
  });

  // Reload memories database
  const refreshBtn = host.querySelector('[data-memory-refresh]');
  refreshBtn?.addEventListener('click', () => {
    void fetchMemoriesFromDb(modal);
  });

  // Add memory row logic
  const addBtn = host.querySelector('[data-memory-add-btn]');
  const addInput = host.querySelector('[data-memory-add-input]');
  addBtn?.addEventListener('click', () => {
    if (addInput instanceof HTMLTextAreaElement) {
      void handleAddMemory(modal, addInput.value.trim());
    }
  });

  // Adjust height of the new memory input
  if (addInput instanceof HTMLTextAreaElement) {
    adjustTextareaHeight(addInput);
    addInput.addEventListener('input', () => {
      adjustTextareaHeight(addInput);
    });
  }

  // Auto-resize card textareas on load & bind save activation on editing
  const items = host.querySelectorAll('[data-memory-card-id]');
  items.forEach(card => {
    const idAttr = card.getAttribute('data-memory-card-id');
    if (!idAttr) return;
    const id = Number(idAttr);
    const textarea = card.querySelector(`[data-memory-content-textarea="${id}"]`);
    const saveBtn = card.querySelector(`[data-memory-save-btn="${id}"]`);
    const deleteBtn = card.querySelector(`[data-memory-delete-btn="${id}"]`);

    if (textarea instanceof HTMLTextAreaElement && saveBtn instanceof HTMLButtonElement) {
      // Set initial heights dynamically based on content
      adjustTextareaHeight(textarea);

      // Keep track of the original DB content value to detect updates
      const originalContent = memoriesState.memories.find(m => m.id === id)?.content || '';
      
      textarea.addEventListener('input', () => {
        adjustTextareaHeight(textarea);
        const hasChanges = textarea.value.trim() !== originalContent.trim() && textarea.value.trim().length > 0;
        saveBtn.disabled = !hasChanges;
        if (hasChanges) {
          saveBtn.classList.add('is-active');
        } else {
          saveBtn.classList.remove('is-active');
        }
      });

      // Save button click
      saveBtn.addEventListener('click', () => {
        void handleUpdateMemory(modal, id, textarea.value.trim());
      });
    }

    // Delete button click
    deleteBtn?.addEventListener('click', () => {
      void handleDeleteMemory(modal, id);
    });
  });
}

// ── Action Handlers ───────────────────────────────────────────────────────────

/**
 * Handles adding a new memory to the SQLite database.
 *
 * @param {HTMLElement} modal - The dialog wrapper element.
 * @param {string} content - Raw memory text content.
 */
async function handleAddMemory(modal, content) {
  if (!content) {
    showError('Memory content cannot be empty.');
    return;
  }

  memoriesState.adding = true;
  renderMemoryStage(modal);

  try {
    const newItem = await IPC.addMemory(content);
    if (newItem && newItem.id) {
      // Add the new item to the beginning of the array so it shows up first
      memoriesState.memories.unshift(newItem);
      showSuccess('Memory added successfully.');
    } else {
      throw new Error('Database operation did not return a valid memory record.');
    }
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to add memory entry.'));
  } finally {
    memoriesState.adding = false;
    renderMemoryStage(modal);
  }
}

/**
 * Handles saving edits of an existing memory to the database.
 *
 * @param {HTMLElement} modal - The dialog wrapper.
 * @param {number} id - Memory ID key.
 * @param {string} newContent - Text input value.
 */
async function handleUpdateMemory(modal, id, newContent) {
  if (!newContent) {
    showError('Memory content cannot be empty.');
    return;
  }

  memoriesState.savingId = id;
  renderMemoryStage(modal);

  try {
    await IPC.updateMemory(id, newContent);
    
    // Update our local state array with the saved content and timestamp
    memoriesState.memories = memoriesState.memories.map(m => {
      if (m.id === id) {
        return {
          ...m,
          content: newContent,
          updatedAt: new Date().toISOString(), // Local timestamp preview until refresh
        };
      }
      return m;
    });

    showSuccess('Memory saved successfully.');
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to update memory content.'));
  } finally {
    memoriesState.savingId = null;
    renderMemoryStage(modal);
  }
}

/**
 * Handles deleting a memory from the database.
 *
 * @param {HTMLElement} modal - The dialog wrapper.
 * @param {number} id - Memory ID.
 */
async function handleDeleteMemory(modal, id) {
  if (memoriesState.savingId === id) return; // Prevent double operations
  
  try {
    await IPC.deleteMemory(id);
    
    // Remove from the local array
    memoriesState.memories = memoriesState.memories.filter(m => m.id !== id);
    
    showSuccess('Memory deleted.');
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to delete memory.'));
  } finally {
    renderMemoryStage(modal);
  }
}

// ── Sizing / Date Formatting Helpers ─────────────────────────────────────────

/**
 * Helper to display human-readable datetime formats.
 * Normalizes SQLite local dates.
 *
 * @param {string} isoString - Date ISO or SQLite timestamp.
 * @returns {string} Formatted localized date.
 */
function formatDate(isoString) {
  if (!isoString) return '';
  try {
    const parsed = new Date(isoString);
    // If standard conversion fails (e.g. missing 'T' separator), attempt inline correction
    if (isNaN(parsed.getTime())) {
      const fixed = isoString.replace(' ', 'T');
      const parsed2 = new Date(fixed);
      if (!isNaN(parsed2.getTime())) {
        return parsed2.toLocaleString();
      }
      return isoString;
    }
    return parsed.toLocaleString();
  } catch (e) {
    return isoString;
  }
}

/**
 * Automatically adjusts the height of a textarea based on its contents,
 * respecting CSS max-height constraints.
 * @param {HTMLTextAreaElement} el
 */
function adjustTextareaHeight(el) {
  if (!el) return;
  el.style.height = 'auto';
  el.style.height = `${el.scrollHeight}px`;
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { resetMemorySettingsState, hydrateMemoryPage };
