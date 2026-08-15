// Models Settings Controller & DOM Coordinator
//
// 1:1 implementation matching Slint models.slint, providers.slint, and setup.slint:
// - View 0: Providers List View with provider cards, active indicators, and status badges.
// - View 1: Provider Setup Form with back button, base URL, API key, discovered models tag box, reload, and save.

import {
  discoverProviderModelsIpc,
  getProviderSetupDetailsIpc,
  getProvidersListIpc,
  saveProviderConfigIpc,
} from './ipc.js';
import type { ProviderSetupDetails, ProviderSummary } from './types.js';

let currentProviders: ProviderSummary[] = [];
let currentSetup: ProviderSetupDetails | null = null;
let activeView = 0; // 0 = List, 1 = Setup
let discoveryDebounceTimer: number | undefined;

/**
 * Initializes the Models Settings panel.
 */
export async function initModelsSettings(): Promise<void> {
  setupBackButton();
  setupSetupFormActions();
  await refreshProvidersList();
}

/**
 * Refreshes the list of supported providers and updates the DOM.
 */
export async function refreshProvidersList(): Promise<void> {
  try {
    currentProviders = await getProvidersListIpc();
    renderProvidersList();
  } catch (err) {
    console.error('[ModelsSettings] Failed to fetch providers list:', err);
  }
}

/**
 * Renders the providers list view cards.
 */
function renderProvidersList(): void {
  const container = document.getElementById('models-providers-container');
  if (!container) return;

  container.innerHTML = '';

  currentProviders.forEach((p) => {
    const card = document.createElement('div');
    card.className = 'provider-card';
    card.dataset.id = p.id;

    const iconClass = getProviderIconClass(p.id);

    card.innerHTML = `
      <div class="provider-icon-wrapper">
        <span class="provider-icon ${iconClass}"></span>
      </div>
      <div class="provider-info">
        <div class="provider-label">${p.label}</div>
        <div class="provider-subinfo">
          <span class="provider-status ${p.status === 'Configured' ? 'configured' : ''}">${p.status}</span>
          <span class="provider-dot"></span>
          <span class="provider-active-model">${p.active_model || '—'}</span>
        </div>
      </div>
      <div class="provider-action">
        ${
          p.is_active
            ? '<span class="provider-active-badge">Active</span>'
            : '<span class="ui-icon icon-chevron-right provider-chevron"></span>'
        }
      </div>
    `;

    card.addEventListener('click', () => {
      openProviderSetup(p.id);
    });

    container.appendChild(card);
  });
}

/**
 * Opens the setup view for a specific provider.
 */
async function openProviderSetup(providerId: string): Promise<void> {
  try {
    currentSetup = await getProviderSetupDetailsIpc(providerId);
    if (!currentSetup) return;

    activeView = 1;
    updateViewSwitch();

    // Populate setup form
    const titleEl = document.getElementById('models-setup-header-title');
    const baseUrlInput = document.getElementById('input-models-base-url') as HTMLInputElement | null;
    const apiKeyInput = document.getElementById('input-models-api-key') as HTMLInputElement | null;

    if (titleEl) {
      titleEl.textContent = `${currentSetup.provider_label} Setup`;
    }
    if (baseUrlInput) {
      baseUrlInput.value = currentSetup.api_base_url;
    }
    if (apiKeyInput) {
      apiKeyInput.value = currentSetup.api_key;
    }

    renderDiscoveredModels(currentSetup.discovered_models, currentSetup.active_model);

    // Auto-fetch available models if credentials exist
    if (currentSetup.api_key.trim().length >= 15 || currentSetup.provider_id === 'ollama') {
      await triggerModelDiscovery();
    }
  } catch (err) {
    console.error('[ModelsSettings] Failed to open provider setup:', err);
  }
}

/**
 * Renders the scrollable box of discovered models.
 */
function renderDiscoveredModels(models: string[], activeModel: string): void {
  const container = document.getElementById('models-discovered-list');
  if (!container) return;

  container.innerHTML = '';

  if (models.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'models-empty-box';
    empty.textContent = 'No models discovered yet. Click "Reload" or enter valid credentials.';
    container.appendChild(empty);
    return;
  }

  models.forEach((m) => {
    const tag = document.createElement('div');
    const isSelected = m === activeModel;
    tag.className = `model-tag-item ${isSelected ? 'selected' : ''}`;
    tag.innerHTML = `
      <span class="model-tag-radio"></span>
      <span class="model-tag-name">${m}</span>
    `;

    tag.addEventListener('click', () => {
      if (currentSetup) {
        currentSetup.active_model = m;
      }
      const tags = container.querySelectorAll('.model-tag-item');
      tags.forEach((t) => t.classList.remove('selected'));
      tag.classList.add('selected');
    });

    container.appendChild(tag);
  });
}

/**
 * Triggers backend dynamic model discovery for the current provider.
 */
async function triggerModelDiscovery(): Promise<void> {
  if (!currentSetup) return;

  const baseUrlInput = document.getElementById('input-models-base-url') as HTMLInputElement | null;
  const apiKeyInput = document.getElementById('input-models-api-key') as HTMLInputElement | null;
  const reloadBtn = document.getElementById('btn-models-reload');

  const base = baseUrlInput ? baseUrlInput.value.trim() : currentSetup.api_base_url;
  const key = apiKeyInput ? apiKeyInput.value.trim() : currentSetup.api_key;

  if (reloadBtn) {
    reloadBtn.classList.add('loading');
    reloadBtn.textContent = 'Fetching...';
  }

  try {
    const discovered = await discoverProviderModelsIpc(currentSetup.provider_id, base, key);
    if (currentSetup) {
      currentSetup.discovered_models = discovered;
      if (!currentSetup.active_model && discovered.length > 0) {
        currentSetup.active_model = discovered[0];
      }
    }
    renderDiscoveredModels(discovered, currentSetup ? currentSetup.active_model : '');
  } catch (err) {
    console.warn('[ModelsSettings] Discovery error:', err);
  } finally {
    if (reloadBtn) {
      reloadBtn.classList.remove('loading');
      reloadBtn.textContent = 'Reload';
    }
  }
}

/**
 * Sets up back button navigation to return to provider list view.
 */
function setupBackButton(): void {
  document.getElementById('btn-models-back')?.addEventListener('click', () => {
    activeView = 0;
    updateViewSwitch();
  });
}

/**
 * Binds setup form buttons (Reload, Save & Activate) and live input listeners.
 */
function setupSetupFormActions(): void {
  const reloadBtn = document.getElementById('btn-models-reload');
  const saveBtn = document.getElementById('btn-models-save');
  const baseUrlInput = document.getElementById('input-models-base-url') as HTMLInputElement | null;
  const apiKeyInput = document.getElementById('input-models-api-key') as HTMLInputElement | null;

  reloadBtn?.addEventListener('click', async () => {
    await triggerModelDiscovery();
  });

  saveBtn?.addEventListener('click', async () => {
    if (!currentSetup) return;

    const base = baseUrlInput ? baseUrlInput.value.trim() : currentSetup.api_base_url;
    const key = apiKeyInput ? apiKeyInput.value.trim() : currentSetup.api_key;
    const selectedModel = currentSetup.active_model;

    try {
      await saveProviderConfigIpc({
        provider_id: currentSetup.provider_id,
        api_base: base,
        api_key: key,
        selected_model: selectedModel,
      });

      // Refresh list and transition back
      await refreshProvidersList();
      activeView = 0;
      updateViewSwitch();
    } catch (err) {
      console.error('[ModelsSettings] Save provider failed:', err);
    }
  });

  // Debounced discovery on credentials input
  const handleInputChange = () => {
    clearTimeout(discoveryDebounceTimer);
    discoveryDebounceTimer = window.setTimeout(async () => {
      const key = apiKeyInput ? apiKeyInput.value.trim() : '';
      if (key.length >= 15 || currentSetup?.provider_id === 'ollama') {
        await triggerModelDiscovery();
      }
    }, 600);
  };

  baseUrlInput?.addEventListener('input', handleInputChange);
  apiKeyInput?.addEventListener('input', handleInputChange);
}

/**
 * Updates UI view container visibility between List and Setup.
 */
function updateViewSwitch(): void {
  const listView = document.getElementById('models-view-list');
  const setupView = document.getElementById('models-view-setup');
  const headerSubtitle = document.getElementById('models-header-subtitle');

  if (activeView === 0) {
    listView?.classList.remove('hidden');
    setupView?.classList.add('hidden');
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure AI models, API keys, endpoints, and provider preferences.';
    }
  } else {
    listView?.classList.add('hidden');
    setupView?.classList.remove('hidden');
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure API credentials, discover available models, and save selections.';
    }
  }
}

/**
 * Maps provider ID to corresponding SVG icon class.
 */
function getProviderIconClass(id: string): string {
  switch (id) {
    case 'open_ai':
      return 'icon-provider-openai';
    case 'anthropic':
      return 'icon-provider-anthropic';
    case 'gemini':
      return 'icon-provider-google';
    case 'groq':
      return 'icon-provider-groq';
    case 'open_router':
      return 'icon-provider-openrouter';
    case 'deep_seek':
      return 'icon-provider-deepseek';
    case 'moonshot':
      return 'icon-provider-moonshot';
    case 'mistral':
      return 'icon-provider-mistral';
    case 'huggingface':
      return 'icon-provider-huggingface';
    case 'ollama':
      return 'icon-provider-ollama';
    case 'qwen':
      return 'icon-provider-qwen';
    case 'nvidia_nim':
      return 'icon-provider-nvidia';
    default:
      return 'icon-provider-custom';
  }
}
