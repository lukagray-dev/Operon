// Settings Sidebar Controller & DOM Renderer
//
// 1:1 visual match with Slint settings navigation:
// - Search filter bar at top with search icon and clear button.
// - Vertical navigation category items with 16x16 SVG mask icons.
// - Left 3px active indicator bar and subtle hover/active highlight.
// - Keyboard arrow navigation between tabs.

import { settingsState } from '../state.js';
import { SETTINGS_CATEGORIES } from './categories.js';
import type { SettingsTabId } from './types.js';

let searchQuery = '';

/**
 * Initializes the Settings Sidebar component.
 */
export function initSettingsSidebar(): void {
  setupSearchInput();
  renderSettingsSidebar();

  // Re-render when active tab changes
  settingsState.subscribe(() => {
    updateActiveCategory();
    renderActivePanel();
  });

  // Setup keyboard navigation
  window.addEventListener('keydown', (e) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      navigateCategory(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      navigateCategory(-1);
    }
  });

  updateActiveCategory();
  renderActivePanel();
}

/**
 * Sets up live search filtering of category items.
 */
function setupSearchInput(): void {
  const input = document.getElementById('settings-search-input') as HTMLInputElement | null;
  const clearBtn = document.getElementById('btn-settings-search-clear');

  input?.addEventListener('input', () => {
    searchQuery = input.value.trim().toLowerCase();
    clearBtn?.classList.toggle('visible', searchQuery.length > 0);
    renderSettingsSidebar();
  });

  clearBtn?.addEventListener('click', () => {
    if (input) {
      input.value = '';
      searchQuery = '';
      clearBtn.classList.remove('visible');
      renderSettingsSidebar();
    }
  });
}

/**
 * Renders the vertical list of settings categories.
 */
export function renderSettingsSidebar(): void {
  const container = document.getElementById('settings-categories-list');
  if (!container) return;

  container.innerHTML = '';

  const filtered = SETTINGS_CATEGORIES.filter((cat) => {
    if (!searchQuery) return true;
    return (
      cat.label.toLowerCase().includes(searchQuery) ||
      cat.description.toLowerCase().includes(searchQuery)
    );
  });

  if (filtered.length === 0) {
    const emptyRow = document.createElement('div');
    emptyRow.className = 'settings-empty-category-row';
    emptyRow.textContent = 'No matching settings';
    container.appendChild(emptyRow);
    return;
  }

  const currentTab = settingsState.getActiveTab();

  filtered.forEach((cat) => {
    const btn = document.createElement('button');
    const isActive = cat.id === currentTab;
    btn.className = `settings-tab-item ${isActive ? 'active' : ''}`;
    btn.dataset.tab = cat.id;

    btn.innerHTML = `
      <div class="settings-tab-active-indicator"></div>
      <span class="ui-icon ${cat.iconClass} settings-tab-icon"></span>
      <span class="settings-tab-label">${cat.label}</span>
      ${cat.badge ? `<span class="settings-tab-badge">${cat.badge}</span>` : ''}
    `;

    btn.addEventListener('click', () => {
      settingsState.setActiveTab(cat.id as SettingsTabId);
    });

    container.appendChild(btn);
  });
}

/**
 * Synchronizes the active highlight class on category items.
 */
function updateActiveCategory(): void {
  const currentTab = settingsState.getActiveTab();
  const items = document.querySelectorAll<HTMLButtonElement>('.settings-tab-item');
  items.forEach((btn) => {
    const isMatch = btn.dataset.tab === currentTab;
    btn.classList.toggle('active', isMatch);
  });
}

/**
 * Toggles visibility of the right panel corresponding to the active category.
 */
function renderActivePanel(): void {
  const currentTab = settingsState.getActiveTab();
  const panels = document.querySelectorAll<HTMLElement>('.settings-panel-view');
  panels.forEach((panel) => {
    panel.classList.toggle('active', panel.dataset.tab === currentTab);
  });

  if (currentTab === 'channels') {
    import('../channels/channels.js').then(({ refreshChannelsData }) => {
      refreshChannelsData();
    }).catch(() => {});
  }

  if (currentTab === 'memory') {
    import('../memory/memory.js').then(({ refreshMemoryData }) => {
      refreshMemoryData();
    }).catch(() => {});
  }
}

/**
 * Advances category selection with Up/Down arrows.
 */
function navigateCategory(offset: number): void {
  const currentTab = settingsState.getActiveTab();
  const currentIndex = SETTINGS_CATEGORIES.findIndex((c) => c.id === currentTab);
  if (currentIndex === -1) return;

  const newIndex = (currentIndex + offset + SETTINGS_CATEGORIES.length) % SETTINGS_CATEGORIES.length;
  const nextCat = SETTINGS_CATEGORIES[newIndex];
  if (nextCat) {
    settingsState.setActiveTab(nextCat.id);
  }
}
