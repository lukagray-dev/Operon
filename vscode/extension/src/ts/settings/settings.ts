// ============================================================================
// Operon Settings Tab UI Coordinator
//
// Lightweight UI controller for the Settings Webview Tab.
// Handles category sidebar navigation, tab panel switching, toggle switches,
// and interactive theme/choice controls.
// ============================================================================

interface SettingsCategory {
  id: string;
  title: string;
  icon: string;
}

const CATEGORIES: SettingsCategory[] = [
  { id: 'general', title: 'General', icon: 'icon-settings-general' },
  { id: 'appearance', title: 'Appearance', icon: 'icon-settings-appearance' },
  { id: 'models', title: 'Models & Providers', icon: 'icon-settings-models' },
  { id: 'permissions', title: 'Permissions', icon: 'icon-settings-permissions' },
  { id: 'channels', title: 'Channels', icon: 'icon-settings-channels' },
  { id: 'skills', title: 'Skills & Tools', icon: 'icon-settings-skills' },
  { id: 'extensions', title: 'Extensions', icon: 'icon-settings-extensions' },
  { id: 'memory', title: 'Memory', icon: 'icon-settings-memory' },
  { id: 'about', title: 'About', icon: 'icon-settings-about' },
];

function initSettingsUI(): void {
  console.log('[Operon Settings] Initializing UI tab switcher...');

  const categoriesContainer = document.getElementById('settings-categories-list');
  const panels = document.querySelectorAll<HTMLElement>('.settings-panel-view');
  const searchInput = document.getElementById('settings-search-input') as HTMLInputElement | null;
  const searchClearBtn = document.getElementById('btn-settings-search-clear');

  let activeTabId = 'general';

  // ── Switch Active Tab ──────────────────────────────────────────────────────
  const switchTab = (tabId: string) => {
    activeTabId = tabId;

    // Update sidebar item active states
    document.querySelectorAll<HTMLElement>('.settings-tab-item').forEach((item) => {
      item.classList.toggle('active', item.getAttribute('data-tab') === tabId);
    });

    // Update panel active states
    panels.forEach((panel) => {
      panel.classList.toggle('active', panel.getAttribute('data-tab') === tabId);
    });
  };

  // ── Render Category Sidebar Items ──────────────────────────────────────────
  if (categoriesContainer) {
    categoriesContainer.innerHTML = '';
    CATEGORIES.forEach((cat) => {
      const item = document.createElement('button');
      item.className = `settings-tab-item${cat.id === activeTabId ? ' active' : ''}`;
      item.setAttribute('data-tab', cat.id);
      item.setAttribute('type', 'button');
      item.innerHTML = `
        <div class="settings-tab-active-indicator"></div>
        <span class="ui-icon settings-tab-icon ${cat.icon}"></span>
        <span class="settings-tab-label">${cat.title}</span>
      `;

      item.addEventListener('click', () => {
        switchTab(cat.id);
      });

      categoriesContainer.appendChild(item);
    });
  }

  // ── Search Filtering ───────────────────────────────────────────────────────
  if (searchInput && searchClearBtn) {
    searchInput.addEventListener('input', () => {
      const q = searchInput.value.trim().toLowerCase();
      searchClearBtn.classList.toggle('visible', q.length > 0);

      document.querySelectorAll<HTMLElement>('.settings-tab-item').forEach((item) => {
        const label = item.querySelector('.settings-tab-label')?.textContent?.toLowerCase() || '';
        item.style.display = label.includes(q) ? '' : 'none';
      });
    });

    searchClearBtn.addEventListener('click', () => {
      searchInput.value = '';
      searchClearBtn.classList.remove('visible');
      document.querySelectorAll<HTMLElement>('.settings-tab-item').forEach((item) => {
        item.style.display = '';
      });
      searchInput.focus();
    });
  }

  // ── Interactive UI Components (Toggle Switches, Segmented Buttons, Theme Cards)
  // Toggle Switches
  document.querySelectorAll<HTMLElement>('.toggle-switch').forEach((toggle) => {
    toggle.addEventListener('click', () => {
      const isChecked = toggle.classList.toggle('checked');
      toggle.setAttribute('aria-checked', String(isChecked));
    });
  });

  // Segmented Choice Buttons
  document.querySelectorAll<HTMLElement>('.segmented-choice').forEach((group) => {
    const buttons = group.querySelectorAll<HTMLElement>('.segmented-choice-btn');
    buttons.forEach((btn) => {
      btn.addEventListener('click', () => {
        buttons.forEach((b) => b.classList.remove('active'));
        btn.classList.add('active');
      });
    });
  });

  // Theme Preview Cards
  document.querySelectorAll<HTMLElement>('.theme-preview-card, .font-preview-card, .orb-selection-card').forEach((card) => {
    card.addEventListener('click', () => {
      const parent = card.parentElement;
      if (parent) {
        parent.querySelectorAll('.selected').forEach((el) => el.classList.remove('selected'));
        card.classList.add('selected');
      }
    });
  });

  // Permissions Sub-Tab Switcher
  const permTabBtns = document.querySelectorAll<HTMLElement>('.seg-choice-perm-tab');
  const permAllowedDirs = document.getElementById('perm-view-allowed-dirs');
  const permGlobalPerms = document.getElementById('perm-view-global-perms');

  permTabBtns.forEach((btn, idx) => {
    btn.addEventListener('click', () => {
      permTabBtns.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');

      if (idx === 0) {
        permAllowedDirs?.classList.remove('hidden');
        permGlobalPerms?.classList.add('hidden');
      } else {
        permAllowedDirs?.classList.add('hidden');
        permGlobalPerms?.classList.remove('hidden');
      }
    });
  });

  // Models Provider View Back / Setup Switcher
  const btnModelsBack = document.getElementById('btn-models-back');
  const modelsListView = document.getElementById('models-view-list');
  const modelsSetupView = document.getElementById('models-view-setup');

  btnModelsBack?.addEventListener('click', () => {
    modelsSetupView?.classList.add('hidden');
    modelsListView?.classList.remove('hidden');
  });

  // Channels Setup Back Navigation
  const btnWaBack = document.getElementById('btn-wa-back');
  const btnTgBack = document.getElementById('btn-tg-back');
  const channelsListView = document.getElementById('channels-view-list');
  const channelsWaView = document.getElementById('channels-view-whatsapp');
  const channelsTgView = document.getElementById('channels-view-telegram');

  btnWaBack?.addEventListener('click', () => {
    channelsWaView?.classList.add('hidden');
    channelsListView?.classList.remove('hidden');
  });

  btnTgBack?.addEventListener('click', () => {
    channelsTgView?.classList.add('hidden');
    channelsListView?.classList.remove('hidden');
  });

  console.log('[Operon Settings] Settings tab ready.');
}

if (document.readyState === 'loading') {
  window.addEventListener('DOMContentLoaded', initSettingsUI);
} else {
  initSettingsUI();
}
