'use strict';

/**
 * extensions.js
 *
 * Extensions settings page for the Operon settings panel.
 *
 * Manages three tabs inside [data-extensions-host]:
 *  - Browse    — search OHub marketplace, download extensions.
 *  - Downloaded — install or delete downloaded extension packages.
 *  - Installed — toggle, authenticate, uninstall, and wipe data.
 *
 * Mirrors the original reference implementation with updated import paths.
 */

import { showError } from '../../shared/toast.js';
import {
  activeCategory,
  renderInlineStatus,
  escapeHtml,
  normalizeErrorMessage,
} from '../settings-panel.js';
import {
  PLACEHOLDER_EXTENSIONS_SEARCH_RESULTS,
  PLACEHOLDER_DOWNLOADED_EXTENSIONS,
  PLACEHOLDER_INSTALLED_EXTENSIONS,
} from './placeholders.js';

// ── Icon Helper ───────────────────────────────────────────────────────────────

/**
 * Returns an <img> tag for an icon from the settings icons directory.
 * @param {string} name - Icon name without extension
 * @param {number} size - Icon size in pixels
 * @returns {string} HTML img tag
 */
function icon(name, size = 14) {
  return `<img src="./assets/icons/settings/${name}.svg" width="${size}" height="${size}" alt="" draggable="false">`;
}

// ── Transient Module State ────────────────────────────────────────────────────

const extensionsState = {
  activeTab: 'browse',
  query: '',
  lastQuery: '',
  browseRows: [],
  downloadedRows: [],
  installedRows: [],
  loadingBrowse: false,
  loadingDownloaded: false,
  loadingInstalled: false,
  actionKey: '',
  status: '',
};

// ── State Reset ───────────────────────────────────────────────────────────────

function resetExtensionsSettingsState() {
  extensionsState.activeTab = 'browse';
  extensionsState.query = '';
  extensionsState.lastQuery = '';
  extensionsState.browseRows = [];
  extensionsState.downloadedRows = [];
  extensionsState.installedRows = [];
  extensionsState.loadingBrowse = false;
  extensionsState.loadingDownloaded = false;
  extensionsState.loadingInstalled = false;
  extensionsState.actionKey = '';
  extensionsState.status = '';
}

// ── Hydration Entry Point ─────────────────────────────────────────────────────

async function hydrateExtensionsPage(modal) {
  if (!modal || activeCategory !== 'extensions') return;

  renderExtensionsStage(modal);

  const preloadTasks = [];
  if (!extensionsState.loadingDownloaded && extensionsState.downloadedRows.length === 0) {
    preloadTasks.push(loadDownloadedExtensions(modal, false));
  }
  if (!extensionsState.loadingInstalled && extensionsState.installedRows.length === 0) {
    preloadTasks.push(loadInstalledExtensions(modal, false));
  }
  if (preloadTasks.length > 0) await Promise.all(preloadTasks);
}

// ── Stage Renderer ────────────────────────────────────────────────────────────

function renderExtensionsStage(modal) {
  const host = modal?.querySelector('[data-extensions-host]');
  if (!host) return;

  const activeTab = normalizeExtensionTab(extensionsState.activeTab);
  extensionsState.activeTab = activeTab;

  host.innerHTML = `
    <div class="settings-extensions__tabs settings-tabs">
      <button class="settings-tabs__item ${activeTab === 'browse' ? 'is-active' : ''}"
              type="button" data-extensions-tab="browse">Browse</button>
      <button class="settings-tabs__item ${activeTab === 'downloaded' ? 'is-active' : ''}"
              type="button" data-extensions-tab="downloaded">Downloaded</button>
      <button class="settings-tabs__item ${activeTab === 'installed' ? 'is-active' : ''}"
              type="button" data-extensions-tab="installed">Installed</button>
    </div>

    <div class="settings-extensions__panel">
      ${activeTab === 'browse'
        ? renderExtensionsBrowseTab()
        : activeTab === 'downloaded'
          ? renderExtensionsDownloadedTab()
          : renderExtensionsInstalledTab()}
    </div>

    ${extensionsState.status
      ? `<div class="settings-extensions__status">${escapeHtml(extensionsState.status)}</div>`
      : ''}
  `;

  bindExtensionsEvents(modal);
}

// ── Browse Tab ────────────────────────────────────────────────────────────────

function renderExtensionsBrowseTab() {
  const loading   = extensionsState.loadingBrowse;
  const query     = extensionsState.query;
  const canSearch = query.trim().length > 0;
  const content   = loading
    ? renderInlineStatus('Searching marketplace...', true)
    : extensionsState.browseRows.length > 0
      ? `
        <div class="settings-extensions__list">
          ${extensionsState.browseRows.map(row => {
            const actionKey       = `download:${extensionIdentity(row.slug, row.version)}`;
            const isDownloading   = extensionsState.actionKey === actionKey;
            const controlsDisabled = Boolean(extensionsState.actionKey) && !isDownloading;
            return `
              <article class="settings-extensions__item">
                <div class="settings-extensions__item-main">
                  <div class="settings-extensions__item-title">
                    <span>${escapeHtml(row.displayName)}</span>
                    <span class="settings-badge">${escapeHtml(row.slug)}</span>
                  </div>
                  <div class="settings-extensions__item-meta">v${escapeHtml(row.version)} | ${escapeHtml(row.registryName)}</div>
                  <div class="settings-extensions__item-summary">${escapeHtml(row.summary)}</div>
                </div>
                <div class="settings-extensions__item-actions">
                  <button class="btn btn--secondary btn--sm" type="button"
                          data-extensions-download="${escapeHtml(row.slug)}"
                          data-extensions-download-version="${escapeHtml(row.version)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${isDownloading
                      ? '<span class="model-selector__spinner" aria-hidden="true"></span> Downloading...'
                      : `${icon('download', 14)} Download`}
                  </button>
                </div>
              </article>
            `;
          }).join('')}
        </div>
      `
      : renderInlineStatus(
          extensionsState.lastQuery
            ? `No extensions found for "${extensionsState.lastQuery}".`
            : 'Search OHub marketplace by slug, name, or summary.'
        );

  return `
    <section class="settings-extensions__section">
      <div class="settings-extensions__toolbar">
        <div class="settings-extensions__search">
          <span class="settings-extensions__search-icon">${icon('search', 14)}</span>
          <input class="settings-extensions__search-input" type="text"
                 data-extensions-search-input
                 value="${escapeHtml(query)}"
                 placeholder="Search extensions (e.g. git, calendar, sql)">
        </div>
        <button class="btn btn--primary btn--sm" type="button"
                data-extensions-search
                ${loading || !canSearch ? 'disabled' : ''}>
          ${loading
            ? '<span class="model-selector__spinner" aria-hidden="true"></span> Searching...'
            : `${icon('search', 14)} Search`}
        </button>
      </div>
      ${content}
    </section>
  `;
}

// ── Downloaded Tab ────────────────────────────────────────────────────────────

function renderExtensionsDownloadedTab() {
  const loading = extensionsState.loadingDownloaded;
  const content = loading
    ? renderInlineStatus('Loading downloaded packages...', true)
    : extensionsState.downloadedRows.length > 0
      ? `
        <div class="settings-extensions__list">
          ${extensionsState.downloadedRows.map(row => {
            const identity         = extensionIdentity(row.slug, row.version);
            const installAction    = `install:${identity}`;
            const deleteAction     = `delete-downloaded:${identity}`;
            const isInstalling     = extensionsState.actionKey === installAction;
            const isDeleting       = extensionsState.actionKey === deleteAction;
            const controlsDisabled = Boolean(extensionsState.actionKey) && !isInstalling && !isDeleting;
            return `
              <article class="settings-extensions__item">
                <div class="settings-extensions__item-main">
                  <div class="settings-extensions__item-title">
                    <span>${escapeHtml(row.displayName)}</span>
                    <span class="settings-badge">${escapeHtml(row.slug)}</span>
                  </div>
                  <div class="settings-extensions__item-meta">v${escapeHtml(row.version)} | ${escapeHtml(row.registryName)} | ${escapeHtml(row.platform)}</div>
                  <div class="settings-extensions__item-summary">${escapeHtml(row.summary)}</div>
                  <div class="settings-extensions__item-footnote">Downloaded ${escapeHtml(row.downloadedAt)}</div>
                </div>
                <div class="settings-extensions__item-actions">
                  <button class="btn btn--primary btn--sm" type="button"
                          data-extensions-install="${escapeHtml(row.slug)}"
                          data-extensions-install-version="${escapeHtml(row.version)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${isInstalling ? '<span class="model-selector__spinner" aria-hidden="true"></span> Installing...' : `${icon('plus', 14)} Install`}
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button"
                          data-extensions-delete-downloaded="${escapeHtml(row.slug)}"
                          data-extensions-delete-downloaded-version="${escapeHtml(row.version)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${isDeleting ? '<span class="model-selector__spinner" aria-hidden="true"></span>' : icon('delete', 14)}
                  </button>
                </div>
              </article>
            `;
          }).join('')}
        </div>
      `
      : renderInlineStatus('No downloaded extension packages.');

  return `
    <section class="settings-extensions__section">
      <div class="settings-extensions__toolbar">
        <span class="settings-extensions__toolbar-title">Downloaded packages ready to install</span>
        <button class="btn btn--ghost btn--sm" type="button"
                data-extensions-refresh-tab="downloaded"
                ${loading ? 'disabled' : ''}>
          ${icon('refresh', 14)} Refresh
        </button>
      </div>
      ${content}
    </section>
  `;
}

// ── Installed Tab ─────────────────────────────────────────────────────────────

function renderExtensionsInstalledTab() {
  const loading = extensionsState.loadingInstalled;
  const content = loading
    ? renderInlineStatus('Loading installed extensions...', true)
    : extensionsState.installedRows.length > 0
      ? `
        <div class="settings-extensions__list">
          ${extensionsState.installedRows.map(row => {
            const toggleAction   = `toggle:${row.slug}`;
            const authAction     = `auth:${row.slug}`;
            const uninstallAction= `uninstall:${row.slug}`;
            const deleteDataAction=`delete-data:${row.slug}`;
            const controlsDisabled = Boolean(extensionsState.actionKey) &&
              ![toggleAction, authAction, uninstallAction, deleteDataAction].includes(extensionsState.actionKey);
            const authLabel = extensionAuthStateLabel(row);

            return `
              <article class="settings-extensions__item">
                <div class="settings-extensions__item-main">
                  <div class="settings-extensions__item-title">
                    <span>${escapeHtml(row.displayName)}</span>
                    <span class="settings-badge">${escapeHtml(row.slug)}</span>
                    <span class="settings-badge ${row.enabled ? 'settings-badge--success' : ''}">
                      ${row.enabled ? 'ON' : 'OFF'}
                    </span>
                  </div>
                  <div class="settings-extensions__item-meta">v${escapeHtml(row.version)} | mcp: ${escapeHtml(row.mcpServerName || 'n/a')}</div>
                  <div class="settings-extensions__item-summary">${escapeHtml(row.summary)}</div>
                  <div class="settings-extensions__item-footnote">Auth: ${escapeHtml(authLabel)}</div>
                </div>
                <div class="settings-extensions__item-actions">
                  <button class="btn btn--secondary btn--sm" type="button"
                          data-extensions-toggle="${escapeHtml(row.slug)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${extensionsState.actionKey === toggleAction
                      ? '<span class="model-selector__spinner" aria-hidden="true"></span>'
                      : icon('power', 14)}
                    ${row.enabled ? 'Disable' : 'Enable'}
                  </button>
                  ${row.authProvider ? `
                    <button class="btn btn--ghost btn--sm" type="button"
                            data-extensions-auth="${escapeHtml(row.slug)}"
                            ${controlsDisabled ? 'disabled' : ''}>
                      ${extensionsState.actionKey === authAction
                        ? '<span class="model-selector__spinner" aria-hidden="true"></span>'
                        : icon('lock', 14)}
                      ${row.authConnected ? 'Re-auth' : 'Authenticate'}
                    </button>
                  ` : ''}
                  <button class="btn btn--ghost btn--sm" type="button"
                          data-extensions-uninstall="${escapeHtml(row.slug)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${extensionsState.actionKey === uninstallAction
                      ? '<span class="model-selector__spinner" aria-hidden="true"></span>'
                      : icon('close', 14)}
                    Uninstall
                  </button>
                  <button class="btn btn--ghost btn--sm" type="button"
                          data-extensions-delete-data="${escapeHtml(row.slug)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${extensionsState.actionKey === deleteDataAction
                      ? '<span class="model-selector__spinner" aria-hidden="true"></span>'
                      : icon('delete', 14)}
                    Data
                  </button>
                </div>
              </article>
            `;
          }).join('')}
        </div>
      `
      : renderInlineStatus('No extensions installed yet.');

  return `
    <section class="settings-extensions__section">
      <div class="settings-extensions__toolbar">
        <span class="settings-extensions__toolbar-title">Installed runtime extensions</span>
        <button class="btn btn--ghost btn--sm" type="button"
                data-extensions-refresh-tab="installed"
                ${loading ? 'disabled' : ''}>
          ${icon('refresh', 14)} Refresh
        </button>
      </div>
      ${content}
    </section>
  `;
}

// ── Event Binding ─────────────────────────────────────────────────────────────

function bindExtensionsEvents(modal) {
  const host = modal?.querySelector('[data-extensions-host]');
  if (!host) return;

  host.querySelectorAll('[data-extensions-tab]').forEach(button => {
    button.addEventListener('click', () => {
      const nextTab = normalizeExtensionTab(button.getAttribute('data-extensions-tab'));
      if (nextTab === extensionsState.activeTab) return;
      extensionsState.activeTab = nextTab;
      renderExtensionsStage(modal);
      if (nextTab === 'downloaded') void loadDownloadedExtensions(modal, false);
      else if (nextTab === 'installed') void loadInstalledExtensions(modal, false);
    });
  });

  const searchInput = host.querySelector('[data-extensions-search-input]');
  if (searchInput instanceof HTMLInputElement) {
    searchInput.addEventListener('input', () => { extensionsState.query = searchInput.value; });
    searchInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') { event.preventDefault(); void searchExtensionsMarketplace(modal); }
    });
  }

  host.querySelector('[data-extensions-search]')?.addEventListener('click', () => {
    void searchExtensionsMarketplace(modal);
  });

  host.querySelectorAll('[data-extensions-refresh-tab]').forEach(button => {
    button.addEventListener('click', () => {
      const tab = normalizeExtensionTab(button.getAttribute('data-extensions-refresh-tab'));
      if (tab === 'downloaded')      void loadDownloadedExtensions(modal, true);
      else if (tab === 'installed')  void loadInstalledExtensions(modal, true);
      else                           void searchExtensionsMarketplace(modal, true);
    });
  });

  host.querySelectorAll('[data-extensions-download]').forEach(button => {
    button.addEventListener('click', () => {
      const slug    = String(button.getAttribute('data-extensions-download') || '').trim();
      const version = String(button.getAttribute('data-extensions-download-version') || '').trim();
      if (!slug) return;
      void handleDownloadExtension(modal, slug, version);
    });
  });

  host.querySelectorAll('[data-extensions-install]').forEach(button => {
    button.addEventListener('click', () => {
      const slug    = String(button.getAttribute('data-extensions-install') || '').trim();
      const version = String(button.getAttribute('data-extensions-install-version') || '').trim();
      if (!slug || !version) return;
      void handleInstallDownloadedExtension(modal, slug, version);
    });
  });

  host.querySelectorAll('[data-extensions-delete-downloaded]').forEach(button => {
    button.addEventListener('click', () => {
      const slug    = String(button.getAttribute('data-extensions-delete-downloaded') || '').trim();
      const version = String(button.getAttribute('data-extensions-delete-downloaded-version') || '').trim();
      if (!slug || !version) return;
      void handleDeleteDownloadedExtension(modal, slug, version);
    });
  });

  host.querySelectorAll('[data-extensions-toggle]').forEach(button => {
    button.addEventListener('click', () => {
      const slug = String(button.getAttribute('data-extensions-toggle') || '').trim();
      if (!slug) return;
      void handleToggleInstalledExtension(modal, slug);
    });
  });

  host.querySelectorAll('[data-extensions-auth]').forEach(button => {
    button.addEventListener('click', () => {
      const slug = String(button.getAttribute('data-extensions-auth') || '').trim();
      if (!slug) return;
      void handleAuthenticateInstalledExtension(modal, slug);
    });
  });

  host.querySelectorAll('[data-extensions-uninstall]').forEach(button => {
    button.addEventListener('click', () => {
      const slug = String(button.getAttribute('data-extensions-uninstall') || '').trim();
      if (!slug) return;
      void handleUninstallInstalledExtension(modal, slug);
    });
  });

  host.querySelectorAll('[data-extensions-delete-data]').forEach(button => {
    button.addEventListener('click', () => {
      const slug = String(button.getAttribute('data-extensions-delete-data') || '').trim();
      if (!slug) return;
      void handleDeleteExtensionData(modal, slug);
    });
  });
}

// ── Async Handlers ────────────────────────────────────────────────────────────

async function searchExtensionsMarketplace(modal, force = false) {
  if (extensionsState.loadingBrowse) return;
  if (extensionsState.actionKey && !force) return;

  const host  = modal?.querySelector('[data-extensions-host]');
  const input = host?.querySelector('[data-extensions-search-input]');
  if (input instanceof HTMLInputElement) extensionsState.query = input.value;

  const query = String(extensionsState.query || '').trim();
  if (!query) { showError('Search query cannot be empty.'); return; }

  if (!force && query === extensionsState.lastQuery && extensionsState.browseRows.length > 0) return;

  extensionsState.loadingBrowse = true;
  extensionsState.status = `Searching marketplace for "${query}"...`;
  renderExtensionsStage(modal);

  try {
    // PLACEHOLDER: Simulate extension search with delay
    await new Promise(resolve => setTimeout(resolve, 300));
    const rows = PLACEHOLDER_EXTENSIONS_SEARCH_RESULTS.filter(ext =>
      ext.slug.toLowerCase().includes(query.toLowerCase()) ||
      ext.displayName.toLowerCase().includes(query.toLowerCase()) ||
      ext.summary.toLowerCase().includes(query.toLowerCase())
    );
    extensionsState.browseRows  = Array.isArray(rows) ? rows.map(normalizeExtensionSearchRow) : [];
    extensionsState.lastQuery   = query;
    extensionsState.status = extensionsState.browseRows.length > 0
      ? `Found ${extensionsState.browseRows.length} extensions for "${query}".`
      : `No extensions found for "${query}".`;
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to search extension marketplace.'));
  } finally {
    extensionsState.loadingBrowse = false;
    renderExtensionsStage(modal);
  }
}

async function loadDownloadedExtensions(modal, force = false) {
  if (extensionsState.loadingDownloaded) return;
  if (!force && extensionsState.downloadedRows.length > 0) { renderExtensionsStage(modal); return; }

  extensionsState.loadingDownloaded = true;
  renderExtensionsStage(modal);

  try {
    // PLACEHOLDER: Load downloaded extensions with delay
    await new Promise(resolve => setTimeout(resolve, 300));
    const rows = PLACEHOLDER_DOWNLOADED_EXTENSIONS;
    extensionsState.downloadedRows = Array.isArray(rows) ? rows.map(normalizeDownloadedExtensionRow) : [];
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to load downloaded extensions.'));
  } finally {
    extensionsState.loadingDownloaded = false;
    renderExtensionsStage(modal);
  }
}

async function loadInstalledExtensions(modal, force = false) {
  if (extensionsState.loadingInstalled) return;
  if (!force && extensionsState.installedRows.length > 0) { renderExtensionsStage(modal); return; }

  extensionsState.loadingInstalled = true;
  renderExtensionsStage(modal);

  try {
    // PLACEHOLDER: Load installed extensions with delay
    await new Promise(resolve => setTimeout(resolve, 300));
    const rows = PLACEHOLDER_INSTALLED_EXTENSIONS;
    extensionsState.installedRows = Array.isArray(rows) ? rows.map(normalizeInstalledExtensionRow) : [];
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to load installed extensions.'));
  } finally {
    extensionsState.loadingInstalled = false;
    renderExtensionsStage(modal);
  }
}

async function withExtensionAction(modal, actionKey, pendingStatus, errorMessage, action) {
  if (extensionsState.actionKey) return null;
  extensionsState.actionKey = actionKey;
  extensionsState.status    = pendingStatus;
  renderExtensionsStage(modal);
  try { return await action(); }
  catch (error) { showError(normalizeErrorMessage(error, errorMessage)); return null; }
  finally { extensionsState.actionKey = ''; renderExtensionsStage(modal); }
}

async function handleDownloadExtension(modal, slug, version) {
  const actionKey = `download:${extensionIdentity(slug, version)}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Downloading ${slug}${version ? `@${version}` : ''}...`,
    'Failed to download extension.',
    async () => {
      // PLACEHOLDER: Simulate extension download
      await new Promise(resolve => setTimeout(resolve, 500));
      const browseRow = extensionsState.browseRows.find(r => r.slug === slug);
      return {
        slug,
        version: version || '1.0.0',
        displayName: browseRow?.displayName || slug,
        summary: browseRow?.summary || 'Extension downloaded',
        registryName: browseRow?.registryName || 'registry',
        platform: 'win32',
        downloadedAt: new Date().toISOString(),
        artifactPath: `/downloads/${slug}@${version}.tar.gz`,
      };
    }
  );
  if (!payload) return;
  const row = normalizeDownloadedExtensionRow(payload);
  extensionsState.status = `Downloaded ${row.slug}@${row.version} (${row.platform})`;
  // Add to downloaded list if not already there
  if (!extensionsState.downloadedRows.find(r => r.slug === slug && r.version === version)) {
    extensionsState.downloadedRows.push(row);
  }
  await loadDownloadedExtensions(modal, true);
}

async function handleInstallDownloadedExtension(modal, slug, version) {
  const actionKey = `install:${extensionIdentity(slug, version)}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Installing ${slug}@${version}...`,
    'Failed to install downloaded extension.',
    async () => {
      // PLACEHOLDER: Simulate extension installation
      await new Promise(resolve => setTimeout(resolve, 500));
      const downloadedRow = extensionsState.downloadedRows.find(r => r.slug === slug && r.version === version);
      return {
        slug,
        version,
        displayName: downloadedRow?.displayName || slug,
        summary: downloadedRow?.summary || 'Extension installed',
        enabled: true,
        mcpServerName: `${slug}-server`,
        authProvider: null,
        authConnected: null,
      };
    }
  );
  if (!payload) return;
  const installed = normalizeInstalledExtensionRow(payload);
  extensionsState.status = `Installed ${installed.slug}@${installed.version}`;
  // Add to installed list if not already there
  if (!extensionsState.installedRows.find(r => r.slug === slug)) {
    extensionsState.installedRows.push(installed);
  }
  await Promise.all([loadDownloadedExtensions(modal, true), loadInstalledExtensions(modal, true)]);
}

async function handleDeleteDownloadedExtension(modal, slug, version) {
  const actionKey = `delete-downloaded:${extensionIdentity(slug, version)}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Deleting downloaded package ${slug}@${version}...`,
    'Failed to delete downloaded extension package.',
    async () => {
      // PLACEHOLDER: Simulate deletion
      await new Promise(resolve => setTimeout(resolve, 300));
      return { message: `Deleted downloaded package ${slug}@${version}` };
    }
  );
  if (!payload) return;
  extensionsState.status = String(payload?.message || `Deleted downloaded package ${slug}@${version}`);
  // Remove from downloaded list
  extensionsState.downloadedRows = extensionsState.downloadedRows.filter(
    r => !(r.slug === slug && r.version === version)
  );
  await loadDownloadedExtensions(modal, true);
}

async function handleToggleInstalledExtension(modal, slug) {
  const actionKey = `toggle:${slug}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Updating extension state for ${slug}...`,
    'Failed to toggle extension state.',
    async () => {
      // PLACEHOLDER: Simulate toggle
      await new Promise(resolve => setTimeout(resolve, 300));
      const ext = extensionsState.installedRows.find(r => r.slug === slug);
      const newEnabled = !ext?.enabled;
      return { enabled: newEnabled };
    }
  );
  if (!payload) return;
  const enabled = Boolean(payload?.enabled);
  extensionsState.status = `Extension ${slug} is now ${enabled ? 'enabled' : 'disabled'}.`;
  // Update in local state
  extensionsState.installedRows = extensionsState.installedRows.map(r =>
    r.slug === slug ? { ...r, enabled } : r
  );
  await loadInstalledExtensions(modal, true);
}

async function handleAuthenticateInstalledExtension(modal, slug) {
  const actionKey = `auth:${slug}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Authenticating ${slug}...`,
    'Failed to authenticate extension.',
    async () => {
      // PLACEHOLDER: Simulate authentication
      await new Promise(resolve => setTimeout(resolve, 400));
      return { message: `Authenticated ${slug}`, authConnected: true };
    }
  );
  if (!payload) return;
  extensionsState.status = String(payload?.message || `Authenticated ${slug}`);
  // Update in local state
  extensionsState.installedRows = extensionsState.installedRows.map(r =>
    r.slug === slug ? { ...r, authConnected: true } : r
  );
  await loadInstalledExtensions(modal, true);
}

async function handleUninstallInstalledExtension(modal, slug) {
  const actionKey = `uninstall:${slug}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Uninstalling ${slug}...`,
    'Failed to uninstall extension.',
    async () => {
      // PLACEHOLDER: Simulate uninstallation
      await new Promise(resolve => setTimeout(resolve, 400));
      return { message: `Uninstalled extension ${slug}` };
    }
  );
  if (!payload) return;
  extensionsState.status = String(payload?.message || `Uninstalled extension ${slug}`);
  // Remove from installed list
  extensionsState.installedRows = extensionsState.installedRows.filter(r => r.slug !== slug);
  await loadInstalledExtensions(modal, true);
}

async function handleDeleteExtensionData(modal, slug) {
  const actionKey = `delete-data:${slug}`;
  const payload   = await withExtensionAction(
    modal, actionKey,
    `Deleting extension data for ${slug}...`,
    'Failed to delete extension data.',
    async () => {
      // PLACEHOLDER: Simulate data deletion
      await new Promise(resolve => setTimeout(resolve, 300));
      return { message: `Deleted extension data for ${slug}` };
    }
  );
  if (!payload) return;
  extensionsState.status = String(payload?.message || `Deleted extension data for ${slug}`);
  await Promise.all([loadInstalledExtensions(modal, true), loadDownloadedExtensions(modal, true)]);
}

// ── Normalizers / Helpers ─────────────────────────────────────────────────────

function normalizeExtensionTab(value) {
  const normalized = String(value || '').trim().toLowerCase();
  if (normalized === 'downloaded' || normalized === 'installed') return normalized;
  return 'browse';
}

function normalizeExtensionSearchRow(row) {
  return {
    slug:         String(row?.slug || '').trim(),
    displayName:  String(row?.displayName || '').trim() || 'Extension',
    summary:      String(row?.summary || '').trim() || 'No summary available.',
    version:      String(row?.version || '').trim() || '0.0.0',
    registryName: String(row?.registryName || '').trim() || 'registry',
  };
}

function normalizeDownloadedExtensionRow(row) {
  return {
    slug:         String(row?.slug || '').trim(),
    displayName:  String(row?.displayName || '').trim() || 'Extension',
    summary:      String(row?.summary || '').trim() || 'No summary available.',
    version:      String(row?.version || '').trim() || '0.0.0',
    registryName: String(row?.registryName || '').trim() || 'registry',
    platform:     String(row?.platform || '').trim() || 'unknown',
    downloadedAt: String(row?.downloadedAt || '').trim() || 'unknown',
    artifactPath: String(row?.artifactPath || '').trim(),
  };
}

function normalizeInstalledExtensionRow(row) {
  const authConnected = row?.authConnected === true ? true : row?.authConnected === false ? false : null;
  return {
    slug:          String(row?.slug || '').trim(),
    displayName:   String(row?.displayName || '').trim() || 'Extension',
    summary:       String(row?.summary || '').trim() || 'No summary available.',
    version:       String(row?.version || '').trim() || '0.0.0',
    enabled:       Boolean(row?.enabled),
    mcpServerName: String(row?.mcpServerName || '').trim(),
    authProvider:  String(row?.authProvider || '').trim(),
    authConnected,
  };
}

function extensionIdentity(slug, version = '') {
  const normalizedSlug    = String(slug || '').trim().toLowerCase();
  const normalizedVersion = String(version || '').trim();
  return normalizedVersion ? `${normalizedSlug}@${normalizedVersion}` : normalizedSlug;
}

function extensionAuthStateLabel(row) {
  if (!row.authProvider)        return 'n/a';
  if (row.authConnected === true)  return `${row.authProvider} (connected)`;
  if (row.authConnected === false) return `${row.authProvider} (not connected)`;
  return `${row.authProvider} (unknown)`;
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { resetExtensionsSettingsState, hydrateExtensionsPage };
