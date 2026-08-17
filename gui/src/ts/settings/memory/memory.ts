// Memory Settings Panel Controller
//
// Architecture mirrors the reference design (settings-main-content/memory.js):
//   - Module-level state object; all renders are full re-renders of #memory-host.
//   - Toolbar: search input with live filter + clear button, icon refresh button.
//   - Memory list: individual dark cards, each with inline auto-grow textarea,
//     a Save button (activates only when content changed), and a Delete button.
//   - Add strip: slim single-line textarea + plus icon button, pinned at bottom.
//   - Tags are shown as read-only chips below each card's textarea (not editable
//     inline — keeps the card slim, matching reference aesthetic).
//
// IPC functions map 1:1 to the Tauri commands in settings/memory/mod.rs.

import { memoryAddIpc, memoryDeleteIpc, memoryEditIpc, memoryListIpc } from './ipc.js';
import type { MemoryEntry } from './types.js';

// ── Module state ──────────────────────────────────────────────────────────────

/** All memories currently loaded from the store (most-recent first). */
const state = {
  memories: [] as MemoryEntry[],
  searchQuery: '',
  loading: false,
  /** ID of the card currently being saved (to show spinner / disable). */
  savingId: null as string | null,
  /** Whether the add operation is in-flight. */
  adding: false,
};

// ── Init ──────────────────────────────────────────────────────────────────────

/**
 * Called once from settings.ts on DOMContentLoaded.
 * Resets state so a fresh fetch occurs next time the panel becomes active.
 */
export async function initMemorySettings(): Promise<void> {
  resetState();
}

/** Resets transient state — called on panel close / re-open. */
function resetState(): void {
  state.memories = [];
  state.searchQuery = '';
  state.loading = false;
  state.savingId = null;
  state.adding = false;
}

// ── Public refresh — called by sidebar.ts when tab becomes active ─────────────

/**
 * Fetches memories from the backend if not yet loaded, then re-renders.
 * Subsequent activations skip the network call (cache-first).
 */
export async function refreshMemoryData(): Promise<void> {
  if (state.memories.length === 0 && !state.loading) {
    await fetchFromStore();
  } else {
    render();
  }
}

// ── Store fetch ───────────────────────────────────────────────────────────────

async function fetchFromStore(): Promise<void> {
  state.loading = true;
  render();

  try {
    const res = await memoryListIpc(200, 0); // load up to 200 at a time
    state.memories = res?.memories ?? [];
  } catch (err) {
    console.error('[MemorySettings] Fetch failed:', err);
    state.memories = [];
  } finally {
    state.loading = false;
    render();
  }
}

// ── Render ────────────────────────────────────────────────────────────────────

/**
 * Full re-render of #memory-host based on current state.
 * Mirrors the reference's renderMemoryStage() pattern.
 */
function render(): void {
  const host = document.getElementById('memory-host');
  if (!host) return;

  // Build toolbar HTML
  const toolbar = `
    <div class="mem-toolbar">
      <div class="mem-search-wrap">
        <span class="mem-search-icon"></span>
        <input
          type="text"
          class="mem-search-input"
          placeholder="Search memories..."
          value="${escHtml(state.searchQuery)}"
          data-mem-search
          spellcheck="false"
          autocomplete="off"
        />
        <button
          class="mem-search-clear"
          data-mem-clear
          title="Clear search"
          style="display:${state.searchQuery ? 'flex' : 'none'}"
        >
          <svg width="10" height="10" viewBox="0 0 16 16" fill="currentColor">
            <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.75.75 0 1 1 1.06 1.06L9.06 8l3.22 3.22a.75.75 0 1 1-1.06 1.06L8 9.06l-3.22 3.22a.75.75 0 0 1-1.06-1.06L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z"/>
          </svg>
        </button>
      </div>
      <button class="mem-refresh-btn" data-mem-refresh title="Reload memories" ${state.loading ? 'disabled' : ''}>
        <svg width="13" height="13" viewBox="0 0 16 16" fill="currentColor">
          <path d="M8 3a5 5 0 1 0 4.546 2.914.5.5 0 0 1 .908-.417A6 6 0 1 1 8 2v1z"/>
          <path d="M8 4.466V.534a.25.25 0 0 1 .41-.192l2.36 1.966c.12.1.12.284 0 .384L8.41 4.658A.25.25 0 0 1 8 4.466z"/>
        </svg>
      </button>
    </div>
  `;

  // Build add strip HTML (pinned at bottom)
  const addStrip = `
    <div class="mem-add-strip">
      <textarea
        class="mem-add-textarea"
        placeholder="Type a new memory to add... (Enter to submit)"
        rows="1"
        data-mem-add-input
        spellcheck="false"
        autocomplete="off"
        ${state.adding ? 'disabled' : ''}
      ></textarea>
      <button class="mem-add-btn" data-mem-add-btn title="Add memory" ${state.adding ? 'disabled' : ''}>
        ${state.adding
          ? '<span class="mem-spinner"></span>'
          : '<svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor"><path d="M7.75 2a.75.75 0 0 1 .75.75V7h4.25a.75.75 0 0 1 0 1.5H8.5v4.25a.75.75 0 0 1-1.5 0V8.5H2.75a.75.75 0 0 1 0-1.5H7V2.75A.75.75 0 0 1 7.75 2Z"/></svg>'
        }
      </button>
    </div>
  `;

  // Loading state
  if (state.loading) {
    host.innerHTML = `
      <div class="mem-container">
        ${toolbar}
        <div class="mem-list-wrap">
          <div class="mem-status-row"><span class="mem-spinner"></span><span>Loading memories...</span></div>
        </div>
        ${addStrip}
      </div>
    `;
    bindEvents(host);
    return;
  }

  // Filter by search query
  const q = state.searchQuery.toLowerCase().trim();
  const filtered = state.memories.filter((m) =>
    m.content.toLowerCase().includes(q)
  );

  // Build cards
  let listHtml = '';
  if (filtered.length === 0) {
    const msg = state.searchQuery
      ? `No memories matching "${escHtml(state.searchQuery)}".`
      : 'No memories stored yet.';
    listHtml = `<div class="mem-status-row"><span>${msg}</span></div>`;
  } else {
    listHtml = filtered.map((m) => {
      const isSaving = state.savingId === m.id;
      const tagsHtml = m.tags.length > 0
        ? `<div class="mem-tags-row">${m.tags.map((t) => `<span class="mem-tag-chip">${escHtml(t)}</span>`).join('')}</div>`
        : '';
      return `
        <div class="mem-card" data-mem-card="${m.id}">
          <div class="mem-card-header">
            <div class="mem-card-meta">
              <span class="mem-card-id">ID: ${escHtml(m.id)}</span>
              <span class="mem-card-time">${formatDate(m.updated_at || m.created_at)}</span>
            </div>
            <div class="mem-card-actions">
              <button
                class="mem-btn-save"
                data-mem-save="${m.id}"
                title="Save changes"
                disabled
              >${isSaving ? '<span class="mem-spinner"></span>' : 'Save'}</button>
              <button
                class="mem-btn-delete"
                data-mem-delete="${m.id}"
                title="Delete memory"
              >
                <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
                  <path d="M11 1.75V3h2.25a.75.75 0 0 1 0 1.5H2.75a.75.75 0 0 1 0-1.5H5V1.75C5 .784 5.784 0 6.75 0h2.5C10.216 0 11 .784 11 1.75ZM4.496 6.675l.66 6.6a.25.25 0 0 0 .249.225h5.19a.25.25 0 0 0 .249-.225l.66-6.6a.75.75 0 0 1 1.492.149l-.66 6.6A1.748 1.748 0 0 1 10.595 15H5.405a1.748 1.748 0 0 1-1.741-1.575l-.66-6.6a.75.75 0 1 1 1.492-.15ZM6.5 1.75V3h3V1.75a.25.25 0 0 0-.25-.25h-2.5a.25.25 0 0 0-.25.25Z"/>
                </svg>
              </button>
            </div>
          </div>
          <div class="mem-card-body">
            <textarea
              class="mem-card-textarea"
              data-mem-textarea="${m.id}"
              rows="1"
              spellcheck="false"
              ${isSaving ? 'disabled' : ''}
            >${escHtml(m.content)}</textarea>
            ${tagsHtml}
          </div>
        </div>
      `;
    }).join('');
  }

  host.innerHTML = `
    <div class="mem-container">
      ${toolbar}
      <div class="mem-list-wrap">${listHtml}</div>
      ${addStrip}
    </div>
  `;

  bindEvents(host);
}

// ── Event binding ─────────────────────────────────────────────────────────────

/**
 * Binds all interactive events after every render.
 * Mirrors bindMemoryPageEvents() from the reference.
 */
function bindEvents(host: HTMLElement): void {
  // Search
  const searchInput = host.querySelector<HTMLInputElement>('[data-mem-search]');
  searchInput?.addEventListener('input', () => {
    state.searchQuery = searchInput.value;
    const clearBtn = host.querySelector<HTMLElement>('[data-mem-clear]');
    if (clearBtn) clearBtn.style.display = searchInput.value ? 'flex' : 'none';
    render();
  });

  // Clear search
  host.querySelector('[data-mem-clear]')?.addEventListener('click', () => {
    state.searchQuery = '';
    render();
  });

  // Refresh
  host.querySelector('[data-mem-refresh]')?.addEventListener('click', () => {
    void fetchFromStore();
  });

  // Add strip
  const addInput = host.querySelector<HTMLTextAreaElement>('[data-mem-add-input]');
  const addBtn = host.querySelector<HTMLButtonElement>('[data-mem-add-btn]');

  if (addInput) {
    autoHeight(addInput);
    addInput.addEventListener('input', () => autoHeight(addInput));
    // Enter (without Shift) submits
    addInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        void handleAdd(addInput.value.trim());
      }
    });
  }
  addBtn?.addEventListener('click', () => {
    void handleAdd(addInput?.value.trim() ?? '');
  });

  // Per-card: textarea auto-height + save activation + save click + delete
  host.querySelectorAll<HTMLElement>('[data-mem-card]').forEach((card) => {
    const id = card.dataset.memCard!;
    const textarea = card.querySelector<HTMLTextAreaElement>(`[data-mem-textarea="${id}"]`);
    const saveBtn = card.querySelector<HTMLButtonElement>(`[data-mem-save="${id}"]`);
    const deleteBtn = card.querySelector<HTMLButtonElement>(`[data-mem-delete="${id}"]`);

    if (textarea && saveBtn) {
      autoHeight(textarea);

      const original = state.memories.find((m) => m.id === id)?.content ?? '';

      textarea.addEventListener('input', () => {
        autoHeight(textarea);
        const dirty = textarea.value.trim() !== original.trim() && textarea.value.trim().length > 0;
        saveBtn.disabled = !dirty;
        saveBtn.classList.toggle('is-dirty', dirty);
      });

      saveBtn.addEventListener('click', () => {
        void handleSave(id, textarea.value.trim());
      });
    }

    deleteBtn?.addEventListener('click', () => {
      void handleDelete(id);
    });
  });
}

// ── Action handlers ───────────────────────────────────────────────────────────

async function handleAdd(content: string): Promise<void> {
  if (!content) return;

  state.adding = true;
  render();

  try {
    const created = await memoryAddIpc(content, []);
    if (created) {
      state.memories.unshift(created); // prepend — most-recent first
    }
  } catch (err) {
    console.error('[MemorySettings] Add failed:', err);
  } finally {
    state.adding = false;
    render();
  }
}

async function handleSave(id: string, newContent: string): Promise<void> {
  if (!newContent) return;

  state.savingId = id;
  render();

  try {
    const updated = await memoryEditIpc(id, newContent, null);
    if (updated) {
      state.memories = state.memories.map((m) =>
        m.id === id ? { ...m, content: newContent, updated_at: new Date().toISOString() } : m
      );
    }
  } catch (err) {
    console.error('[MemorySettings] Save failed:', err);
  } finally {
    state.savingId = null;
    render();
  }
}

async function handleDelete(id: string): Promise<void> {
  if (state.savingId === id) return; // guard against concurrent ops

  try {
    await memoryDeleteIpc(id);
    state.memories = state.memories.filter((m) => m.id !== id);
  } catch (err) {
    console.error('[MemorySettings] Delete failed:', err);
  } finally {
    render();
  }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/**
 * Auto-grows a textarea to fit its content, capped by CSS max-height.
 * Mirrors adjustTextareaHeight() from the reference.
 */
function autoHeight(el: HTMLTextAreaElement): void {
  el.style.height = 'auto';
  el.style.height = `${el.scrollHeight}px`;
}

/** Escapes a string for safe HTML interpolation. */
function escHtml(s: string): string {
  return String(s ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Formats an RFC3339 or SQLite timestamp as a locale string.
 * Handles the SQLite "space" separator variant (no T between date and time).
 */
function formatDate(raw: string): string {
  if (!raw) return '';
  try {
    const d = new Date(raw);
    if (isNaN(d.getTime())) {
      const fixed = new Date(raw.replace(' ', 'T'));
      return isNaN(fixed.getTime()) ? raw : fixed.toLocaleString();
    }
    return d.toLocaleString();
  } catch {
    return raw;
  }
}
