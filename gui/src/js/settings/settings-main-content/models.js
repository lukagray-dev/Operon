'use strict';

/**
 * models.js
 *
 * Models settings page for the Operon settings panel.
 *
 * Manages two views inside the [data-models-host] container:
 *  1. List view  — shows all available model providers as clickable cards.
 *  2. Setup view — shows API base URL, API key, model discovery, and save controls
 *                  for the selected provider.
 *
 * All IPC calls go through the shared ipc module.
 * Transient UI state is held in modelsState (local, not in the store).
 */

import { showError, showSuccess } from '../../shared/toast.js';
import {
  activeCategory,
  renderInlineStatus,
  buildSettingRow,
  escapeHtml,
  normalizeErrorMessage,
} from '../settings-panel.js';
import * as IPC from '../../shared/ipc.js';

// ── Provider Icon Mapping ─────────────────────────────────────────────────────

/**
 * Maps provider IDs to the icon filename located in
 * Operon/gui/src/assets/icons/ (the parent icons folder, not settings/).
 * The ICONS module in the reference project had named functions for these;
 * here we use <img> tags pointing to the provider SVG files.
 *
 * @type {Object<string, string>}
 */
const PROVIDER_ICON_PATHS = {
  openai:     './assets/icons/settings/provider-openai.svg',
  anthropic:  './assets/icons/settings/provider-anthropic.svg',
  google:     './assets/icons/settings/provider-google.svg',
  groq:       './assets/icons/settings/provider-groq.svg',
  openrouter: './assets/icons/settings/provider-openrouter.svg',
  deepseek:   './assets/icons/settings/provider-deepseek.svg',
  moonshot:   './assets/icons/settings/provider-moonshot.svg',
  mistral:    './assets/icons/settings/provider-mistral.svg',
  huggingface:'./assets/icons/settings/provider-huggingface.svg',
  ollama:     './assets/icons/settings/provider-ollama.svg',
  qwen:       './assets/icons/settings/provider-qwen.svg',
  nvidia:     './assets/icons/settings/provider-nvidia.svg',
  custom:     './assets/icons/settings/provider-custom.svg',
};

// ── Transient Module State ────────────────────────────────────────────────────

/**
 * All mutable state for the Models page lives here.
 * Reset on dialog close via resetModelsSettingsState().
 */
const modelsState = {
  /** @type {Array<Object>} Normalized provider summary rows */
  providers: [],
  /** @type {string} ID of the provider currently open in setup view */
  selectedProviderId: '',
  /** @type {'list'|'setup'} Current view inside the host container */
  activeView: 'list',
  /** @type {Map<string, Object>} Full setup data keyed by provider ID */
  setupByProvider: new Map(),
  /** @type {Map<string, string[]>} Discovered model list keyed by provider ID */
  discoveredByProvider: new Map(),
  /** @type {Map<string, Object>} Unsaved form draft keyed by provider ID */
  draftByProvider: new Map(),
  /** Flags for loading / saving states */
  loadingProviders: false,
  loadingSetup: false,
  loadingDiscovery: false,
  saving: false,
};

// ── State Reset ───────────────────────────────────────────────────────────────

/**
 * Clears all transient modelsState fields.
 * Called by settings-panel.js when the dialog is closed.
 */
function resetModelsSettingsState() {
  modelsState.providers = [];
  modelsState.selectedProviderId = '';
  modelsState.activeView = 'list';
  modelsState.setupByProvider.clear();
  modelsState.discoveredByProvider.clear();
  modelsState.draftByProvider.clear();
  modelsState.loadingProviders = false;
  modelsState.loadingSetup = false;
  modelsState.loadingDiscovery = false;
  modelsState.saving = false;
}

// ── Hydration Entry Point ─────────────────────────────────────────────────────

/**
 * Called by settings-panel.js after the Models page scaffold is injected.
 * Loads the provider list if not already loaded, then renders the current view.
 *
 * @param {HTMLElement} modal - The settings dialog root element.
 */
async function hydrateModelsPage(modal) {
  // Guard: don't hydrate if the user already navigated away
  if (!modal || activeCategory !== 'models') return;

  if (modelsState.providers.length === 0 && !modelsState.loadingProviders) {
    modelsState.loadingProviders = true;
    renderModelsStage(modal);
    try {
      // Load model providers from backend
      const rows = await IPC.getModelProviders();
      modelsState.providers = Array.isArray(rows) ? rows.map(normalizeProviderRow) : [];
    } catch (error) {
      modelsState.providers = [];
      showError(normalizeErrorMessage(error, 'Failed to load model providers.'));
    } finally {
      modelsState.loadingProviders = false;
    }
  }

  // If the previously selected provider no longer exists, reset to list view
  if (
    modelsState.selectedProviderId &&
    !modelsState.providers.some(r => r.id === modelsState.selectedProviderId)
  ) {
    modelsState.selectedProviderId = '';
    modelsState.activeView = 'list';
  }

  renderModelsStage(modal);
  if (modelsState.activeView === 'setup' && modelsState.selectedProviderId) {
    await loadProviderSetup(modal, modelsState.selectedProviderId);
  }
}

// ── Stage Renderer ────────────────────────────────────────────────────────────

/**
 * Re-renders the inside of [data-models-host] based on current modelsState.
 * @param {HTMLElement} modal - Dialog root element.
 */
function renderModelsStage(modal) {
  const host    = modal?.querySelector('[data-models-host]');
  const titleEl = modal?.querySelector('[data-models-title]');
  const descEl  = modal?.querySelector('[data-models-description]');
  if (!host) return;

  // Reset header text for list view
  if (titleEl) titleEl.textContent = 'Models';
  if (descEl)  descEl.textContent  = 'Configure model providers, credentials, and active model selection';

  if (modelsState.loadingProviders) {
    host.innerHTML = `<div class="settings-models__view">${renderInlineStatus('Loading providers...', true)}</div>`;
    return;
  }

  if (modelsState.providers.length === 0) {
    host.innerHTML = `<div class="settings-models__view">${renderInlineStatus('No providers available.')}</div>`;
    return;
  }

  if (modelsState.activeView === 'setup' && modelsState.selectedProviderId) {
    const provider = modelsState.providers.find(r => r.id === modelsState.selectedProviderId);
    if (provider && titleEl) {
      titleEl.innerHTML = `Models <span class="settings-title-separator">|</span> ${escapeHtml(provider.label)}`;
    }
    host.innerHTML = renderProviderSetupView();
    bindProviderSetupEvents(modal);
    return;
  }

  host.innerHTML = renderProvidersListView();
  bindProviderListEvents(modal);
}

// ── List View ─────────────────────────────────────────────────────────────────

/**
 * Renders the provider list as clickable cards.
 * @returns {string} HTML.
 */
function renderProvidersListView() {
  const rows = modelsState.providers.map(provider => {
    const isSelected = provider.id === modelsState.selectedProviderId && modelsState.activeView === 'setup';
    const iconPath   = PROVIDER_ICON_PATHS[provider.id] || PROVIDER_ICON_PATHS.custom;
    const iconHtml   = `<img src="${iconPath}" alt="${escapeHtml(provider.label)}" width="18" height="18" draggable="false">`;

    const statusLabel = provider.isConfigured
      ? 'Configured'
      : (provider.requiresApiKey ? 'API key required' : 'Not configured');
    const modelLabel = provider.activeModel || 'No model selected';

    return `
      <button class="settings-models__provider-card ${isSelected ? 'is-selected' : ''}"
              data-model-provider-open="${escapeHtml(provider.id)}"
              type="button">
        <span class="settings-models__provider-icon">${iconHtml}</span>
        <span class="settings-models__provider-content">
          <span class="settings-models__provider-title">${escapeHtml(provider.label)}</span>
          <span class="settings-models__provider-meta">${escapeHtml(statusLabel)}</span>
          <span class="settings-models__provider-model">${escapeHtml(modelLabel)}</span>
        </span>
        ${provider.isActive
          ? '<span class="settings-models__provider-pill">Active</span>'
          : `<span class="settings-models__provider-arrow" aria-hidden="true">
               <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
                    fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                 <polyline points="9 18 15 12 9 6"/>
               </svg>
             </span>`}
      </button>
    `;
  }).join('');

  return `<div class="settings-models__view settings-models__view--list">${rows}</div>`;
}

/**
 * Binds click events to provider card buttons in the list view.
 * @param {HTMLElement} modal - Dialog root element.
 */
function bindProviderListEvents(modal) {
  modal?.querySelectorAll('[data-model-provider-open]').forEach(button => {
    button.addEventListener('click', () => {
      const id = button.getAttribute('data-model-provider-open') || '';
      if (!id) return;
      void openProviderSetup(modal, id);
    });
  });
}

// ── Setup View ────────────────────────────────────────────────────────────────

/**
 * Renders the provider setup form (back button, API fields, model selector, save).
 * @returns {string} HTML.
 */
function renderProviderSetupView() {
  const providerId = modelsState.selectedProviderId;
  if (!providerId) {
    return `<div class="settings-models__view settings-models__view--setup">${renderInlineStatus('Select a provider to configure model setup.')}</div>`;
  }

  const provider = modelsState.providers.find(r => r.id === providerId);
  if (!provider) {
    return `<div class="settings-models__view settings-models__view--setup">${renderInlineStatus('Provider metadata is unavailable.')}</div>`;
  }

  const topbar = `
    <div class="settings-models__topbar">
      <button class="btn btn--ghost btn--sm" type="button" data-models-back>
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
             fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="15 18 9 12 15 6"/>
        </svg>
        Back
      </button>
      <span class="settings-models__topbar-title">${escapeHtml(provider.label)} setup</span>
    </div>
  `;

  if (modelsState.loadingSetup) {
    return `<div class="settings-models__view settings-models__view--setup">${topbar}${renderInlineStatus('Loading provider setup...', true)}</div>`;
  }

  const setup = modelsState.setupByProvider.get(providerId);
  if (!setup) {
    return `<div class="settings-models__view settings-models__view--setup">${topbar}${renderInlineStatus('Provider setup not loaded.')}</div>`;
  }

  const existingDiscovered = modelsState.discoveredByProvider.get(providerId) || [];
  const fallbackModels     = sanitizeModelList(setup.fallbackModels);
  const discoveredModels   = existingDiscovered.length > 0 ? existingDiscovered : fallbackModels;
  if (existingDiscovered.length === 0 && fallbackModels.length > 0) {
    modelsState.discoveredByProvider.set(providerId, fallbackModels);
  }

  const draft         = getProviderDraft(providerId, setup, discoveredModels);
  const selectedModel = draft.model || setup.selectedModel || discoveredModels[0] || '';

  const docsLink = setup.docsUrl
    ? `<a href="${escapeHtml(setup.docsUrl)}" target="_blank" rel="noopener noreferrer">Docs</a>`
    : '';

  // Update the page header description to include a docs link
  const descEl = document.querySelector('[data-models-description]');
  if (descEl) {
    descEl.innerHTML = `Use provider credentials to discover and save model selection. ${docsLink}`;
  }

  return `
    <div class="settings-models__view settings-models__view--setup">
      ${topbar}

      ${buildSettingRow('API Base URL', 'Base URL for the provider API', `
        <input class="settings-models__input"
               type="text"
               data-model-api-base
               value="${escapeHtml(draft.apiBase || setup.apiBase || setup.defaultApiBase)}"
               placeholder="${escapeHtml(setup.defaultApiBase)}">
      `)}

      ${buildSettingRow('API Key', setup.requiresApiKey ? 'Required for this provider' : 'Optional for this provider', `
        <input class="settings-models__input"
               type="password"
               data-model-api-key
               value="${escapeHtml(draft.apiKey || setup.apiKey)}"
               placeholder="${setup.requiresApiKey ? 'Required' : 'Optional'}">
      `)}

      ${buildSettingRow('Fetch models', 'Fetch available models using current settings', `
        <button class="btn btn--secondary btn--sm"
                type="button"
                data-model-discover
                ${modelsState.loadingDiscovery ? 'disabled' : ''}>
          ${modelsState.loadingDiscovery
            ? '<span class="model-selector__spinner" aria-hidden="true"></span> Fetching...'
            : 'Fetch'}
        </button>
      `)}

      ${buildSettingRow('Discovered models', 'Select from fetched models', `
        <select class="setting-select settings-models__select"
                data-model-discovered-select
                ${discoveredModels.length === 0 ? 'disabled' : ''}>
          ${discoveredModels.length === 0
            ? '<option value="">No models discovered yet</option>'
            : discoveredModels.map(m => `<option value="${escapeHtml(m)}" ${m === selectedModel ? 'selected' : ''}>${escapeHtml(m)}</option>`).join('')}
        </select>
      `)}

      ${buildSettingRow('Active model', 'Enter or select the model to use', `
        <input class="settings-models__input"
               type="text"
               data-model-name
               value="${escapeHtml(selectedModel)}"
               placeholder="Enter model id">
      `)}

      <div class="settings-row">
        <div class="settings-row__info"></div>
        <div class="settings-row__control" style="gap: 8px;">
          <button class="btn btn--ghost btn--sm"
                  type="button"
                  data-model-refresh
                  ${modelsState.loadingSetup || modelsState.loadingDiscovery ? 'disabled' : ''}>
            Reload
          </button>
          <button class="btn btn--primary btn--sm"
                  type="button"
                  data-model-save
                  ${modelsState.saving ? 'disabled' : ''}>
            ${modelsState.saving
              ? '<span class="model-selector__spinner" aria-hidden="true"></span> Saving...'
              : 'Save & Activate'}
          </button>
        </div>
      </div>
    </div>
  `;
}

/**
 * Binds all events for the provider setup view.
 * @param {HTMLElement} modal - Dialog root element.
 */
function bindProviderSetupEvents(modal) {
  const host = modal?.querySelector('[data-models-host]');
  if (!host) return;

  host.querySelector('[data-models-back]')?.addEventListener('click', () => {
    goBackToProviderList(modal);
  });

  const discoveredSelect = host.querySelector('[data-model-discovered-select]');
  discoveredSelect?.addEventListener('change', () => {
    const modelInput = host.querySelector('[data-model-name]');
    if (modelInput && discoveredSelect instanceof HTMLSelectElement) {
      modelInput.value = discoveredSelect.value;
      syncProviderDraftFromInputs(modal);
    }
  });

  host.querySelector('[data-model-api-base]')?.addEventListener('input', () => syncProviderDraftFromInputs(modal));
  host.querySelector('[data-model-api-key]')?.addEventListener('input', () => syncProviderDraftFromInputs(modal));
  host.querySelector('[data-model-name]')?.addEventListener('input', () => syncProviderDraftFromInputs(modal));

  host.querySelector('[data-model-discover]')?.addEventListener('click', () => {
    void discoverModelsForSelectedProvider(modal);
  });

  host.querySelector('[data-model-refresh]')?.addEventListener('click', () => {
    void loadProviderSetup(modal, modelsState.selectedProviderId, true);
  });

  host.querySelector('[data-model-save]')?.addEventListener('click', () => {
    void saveSelectedProviderSetup(modal);
  });
}

// ── Navigation ────────────────────────────────────────────────────────────────

async function openProviderSetup(modal, providerId) {
  modelsState.selectedProviderId = providerId;
  modelsState.activeView = 'setup';
  renderModelsStage(modal);
  await loadProviderSetup(modal, providerId);
}

function goBackToProviderList(modal) {
  syncProviderDraftFromInputs(modal);
  modelsState.activeView = 'list';
  renderModelsStage(modal);
}

// ── Data Loading / Saving ─────────────────────────────────────────────────────

async function loadProviderSetup(modal, providerId, force = false) {
  if (!modal || !providerId) { renderModelsStage(modal); return; }
  if (!force && modelsState.setupByProvider.has(providerId)) { renderModelsStage(modal); return; }

  modelsState.loadingSetup = true;
  renderModelsStage(modal);

  try {
    // Load provider setup from backend
    const setup = await IPC.getModelProviderSetup(providerId);
    const normalized = normalizeProviderSetup(setup);
    modelsState.setupByProvider.set(providerId, normalized);
    syncProviderSummary(providerId, normalized);

    const fallback = sanitizeModelList(normalized.fallbackModels);
    if (!modelsState.discoveredByProvider.has(providerId) && fallback.length > 0) {
      modelsState.discoveredByProvider.set(providerId, fallback);
    }

    if (force || !modelsState.draftByProvider.has(providerId)) {
      modelsState.draftByProvider.set(providerId, {
        apiBase: normalized.apiBase || normalized.defaultApiBase,
        apiKey:  normalized.apiKey,
        model:   normalized.selectedModel || fallback[0] || '',
      });
    }
  } catch (error) {
    console.error('Failed to load provider setup for models page:', {
      providerId,
      error,
    });
    showError(normalizeErrorMessage(error, 'Failed to load provider setup.'));
  } finally {
    modelsState.loadingSetup = false;
    renderModelsStage(modal);
  }
}

function getProviderDraft(providerId, setup, discoveredModels = []) {
  const existing = modelsState.draftByProvider.get(providerId);
  if (existing) return existing;
  const seeded = {
    apiBase: setup.apiBase || setup.defaultApiBase,
    apiKey:  setup.apiKey,
    model:   setup.selectedModel || discoveredModels[0] || '',
  };
  modelsState.draftByProvider.set(providerId, seeded);
  return seeded;
}

function syncProviderDraftFromInputs(modal) {
  const providerId = modelsState.selectedProviderId;
  if (!providerId) return null;

  const stageHost = modal?.querySelector('[data-models-host]');
  if (!stageHost) return null;

  const setup    = modelsState.setupByProvider.get(providerId);
  const fallback = { apiBase: setup?.apiBase || '', apiKey: setup?.apiKey || '', model: setup?.selectedModel || '' };
  const draft    = modelsState.draftByProvider.get(providerId) || fallback;

  const apiBaseEl = stageHost.querySelector('[data-model-api-base]');
  const apiKeyEl  = stageHost.querySelector('[data-model-api-key]');
  const modelEl   = stageHost.querySelector('[data-model-name]');

  if (apiBaseEl instanceof HTMLInputElement) draft.apiBase = apiBaseEl.value.trim();
  if (apiKeyEl  instanceof HTMLInputElement) draft.apiKey  = apiKeyEl.value.trim();
  if (modelEl   instanceof HTMLInputElement) draft.model   = modelEl.value.trim();

  modelsState.draftByProvider.set(providerId, draft);
  return { ...draft };
}

async function discoverModelsForSelectedProvider(modal) {
  const providerId = modelsState.selectedProviderId;
  if (!providerId || modelsState.loadingDiscovery) return;

  const form = readProviderSetupForm(modal);
  modelsState.loadingDiscovery = true;
  renderModelsStage(modal);

  try {
    // Call backend to discover models
    const payload = await IPC.discoverModels({
      providerId,
      apiBase: form.apiBase,
      apiKey: form.apiKey,
    });

    if (!payload || !Array.isArray(payload.models)) {
      throw new Error('Model discovery returned an invalid response.');
    }

    const discoveredModelIds = payload.models
      .map(m => String(m?.modelId || '').trim())
      .filter(Boolean);
    modelsState.discoveredByProvider.set(providerId, discoveredModelIds);

    // Update setup with discovered models
    const setup = modelsState.setupByProvider.get(providerId);
    if (setup) {
      const preferred = String(payload?.activeModel || form.model || setup.selectedModel || '').trim();
      setup.selectedModel = preferred || (discoveredModelIds[0] ?? '');
      setup.apiBase = form.apiBase || setup.defaultApiBase;
      setup.apiKey  = form.apiKey;
      modelsState.setupByProvider.set(providerId, setup);
      syncProviderSummary(providerId, setup);
    }

    const draft = modelsState.draftByProvider.get(providerId) || { apiBase: '', apiKey: '', model: '' };
    draft.apiBase = form.apiBase;
    draft.apiKey  = form.apiKey;
    if (setup?.selectedModel) draft.model = setup.selectedModel;
    modelsState.draftByProvider.set(providerId, draft);
    
    showSuccess('Models discovered successfully.');
  } catch (error) {
    console.error('Failed to discover models for provider setup:', {
      providerId,
      apiBase: form.apiBase,
      apiKeyProvided: Boolean(form.apiKey),
      error,
    });
    showError(normalizeErrorMessage(error, 'Failed to discover models.'));
  } finally {
    modelsState.loadingDiscovery = false;
    renderModelsStage(modal);
  }
}

async function saveSelectedProviderSetup(modal) {
  const providerId = modelsState.selectedProviderId;
  if (!providerId || modelsState.saving) return;

  const form = readProviderSetupForm(modal);
  if (!form.model) { showError('Model cannot be empty.'); return; }

  modelsState.saving = true;
  renderModelsStage(modal);

  try {
    // Call backend to save provider setup
    const payload = await IPC.saveProviderSetup({
      providerId,
      apiBase: form.apiBase,
      apiKey: form.apiKey,
      model: form.model,
    });

    const resolvedModel = String(payload?.model || form.model).trim() || form.model;

    const setup = modelsState.setupByProvider.get(providerId);
    if (setup) {
      setup.apiBase        = form.apiBase || setup.defaultApiBase;
      setup.apiKey         = form.apiKey;
      setup.selectedModel  = resolvedModel;
      setup.isActive       = true;
      modelsState.setupByProvider.set(providerId, setup);
      syncProviderSummary(providerId, setup);
    }

    modelsState.providers = modelsState.providers.map(p => ({
      ...p,
      isActive: p.id === providerId,
      isConfigured: p.id === providerId
        ? (p.requiresApiKey ? Boolean(form.apiKey.trim()) : true)
        : p.isConfigured,
      activeModel: p.id === providerId ? resolvedModel : p.activeModel,
    }));

    const discovered = modelsState.discoveredByProvider.get(providerId) || [];
    if (!discovered.includes(resolvedModel)) {
      modelsState.discoveredByProvider.set(providerId, [resolvedModel, ...discovered]);
    }
    modelsState.draftByProvider.set(providerId, { apiBase: form.apiBase, apiKey: form.apiKey, model: resolvedModel });
    
    showSuccess('Model provider setup saved and activated.');
  } catch (error) {
    console.error('Failed to save provider setup:', {
      providerId,
      apiBase: form.apiBase,
      model: form.model,
      apiKeyProvided: Boolean(form.apiKey),
      error,
    });
    showError(normalizeErrorMessage(error, 'Failed to save provider setup.'));
  } finally {
    modelsState.saving = false;
    renderModelsStage(modal);
  }
}

function readProviderSetupForm(modal) {
  const draft = syncProviderDraftFromInputs(modal);
  if (draft) return draft;
  const fallback = modelsState.draftByProvider.get(modelsState.selectedProviderId);
  return fallback
    ? { apiBase: String(fallback.apiBase || '').trim(), apiKey: String(fallback.apiKey || '').trim(), model: String(fallback.model || '').trim() }
    : { apiBase: '', apiKey: '', model: '' };
}

function syncProviderSummary(providerId, setup) {
  modelsState.providers = modelsState.providers.map(p =>
    p.id !== providerId ? p : {
      ...p,
      activeModel:  setup.selectedModel,
      isActive:     setup.isActive,
      isConfigured: p.requiresApiKey ? Boolean(setup.apiKey.trim()) : true,
    }
  );
}

// ── Normalizers ───────────────────────────────────────────────────────────────

function normalizeProviderRow(row) {
  return {
    id:             String(row?.id || '').trim(),
    label:          String(row?.label || '').trim() || 'Provider',
    defaultApiBase: String(row?.defaultApiBase || '').trim(),
    docsUrl:        String(row?.docsUrl || '').trim(),
    requiresApiKey: Boolean(row?.requiresApiKey),
    isActive:       Boolean(row?.isActive),
    isConfigured:   Boolean(row?.isConfigured),
    activeModel:    String(row?.activeModel || '').trim(),
  };
}

function normalizeProviderSetup(row) {
  return {
    providerId:     String(row?.providerId || '').trim(),
    label:          String(row?.label || '').trim() || 'Provider',
    defaultApiBase: String(row?.defaultApiBase || '').trim(),
    docsUrl:        String(row?.docsUrl || '').trim(),
    requiresApiKey: Boolean(row?.requiresApiKey),
    apiBase:        String(row?.apiBase || '').trim(),
    apiKey:         String(row?.apiKey || '').trim(),
    selectedModel:  String(row?.selectedModel || '').trim(),
    fallbackModels: Array.isArray(row?.fallbackModels) ? row.fallbackModels : [],
    isActive:       Boolean(row?.isActive),
  };
}

function sanitizeModelList(values) {
  if (!Array.isArray(values)) return [];
  const deduped = new Set();
  const out     = [];
  for (const entry of values) {
    const m = String(entry || '').trim();
    if (!m || deduped.has(m)) continue;
    deduped.add(m);
    out.push(m);
  }
  return out;
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { resetModelsSettingsState, hydrateModelsPage };
