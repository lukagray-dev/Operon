'use strict';

/**
 * skills.js
 *
 * Skills settings page for the Operon settings panel.
 *
 * Manages two tabs inside [data-skills-host]:
 *  - Browse — search OHub skills by query, install one-click.
 *  - Installed — list and uninstall workspace skills.
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
  PLACEHOLDER_SKILLS_SEARCH_RESULTS,
  PLACEHOLDER_INSTALLED_SKILLS,
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

const skillsState = {
  activeTab: 'browse',
  query: '',
  lastQuery: '',
  browseRows: [],
  installedRows: [],
  loadingBrowse: false,
  loadingInstalled: false,
  actionKey: '',
  status: '',
};

// ── State Reset ───────────────────────────────────────────────────────────────

function resetSkillsSettingsState() {
  skillsState.activeTab = 'browse';
  skillsState.query = '';
  skillsState.lastQuery = '';
  skillsState.browseRows = [];
  skillsState.installedRows = [];
  skillsState.loadingBrowse = false;
  skillsState.loadingInstalled = false;
  skillsState.actionKey = '';
  skillsState.status = '';
}

// ── Hydration Entry Point ─────────────────────────────────────────────────────

async function hydrateSkillsPage(modal) {
  if (!modal || activeCategory !== 'skills') return;

  renderSkillsStage(modal);

  if (!skillsState.loadingInstalled && skillsState.installedRows.length === 0) {
    await loadInstalledSkills(modal, false);
  }
}

// ── Stage Renderer ────────────────────────────────────────────────────────────

function renderSkillsStage(modal) {
  const host = modal?.querySelector('[data-skills-host]');
  if (!host) return;

  const activeTab = normalizeSkillTab(skillsState.activeTab);
  skillsState.activeTab = activeTab;

  host.innerHTML = `
    <div class="settings-skills__tabs settings-tabs">
      <button class="settings-tabs__item ${activeTab === 'browse' ? 'is-active' : ''}"
              type="button" data-skills-tab="browse">
        Browse
      </button>
      <button class="settings-tabs__item ${activeTab === 'installed' ? 'is-active' : ''}"
              type="button" data-skills-tab="installed">
        Installed
      </button>
    </div>

    <div class="settings-skills__panel">
      ${activeTab === 'browse'
        ? renderSkillsBrowseTab()
        : renderSkillsInstalledTab()}
    </div>

    ${skillsState.status ? `<div class="settings-skills__status">${escapeHtml(skillsState.status)}</div>` : ''}
  `;

  bindSkillsEvents(modal);
}

// ── Browse Tab ────────────────────────────────────────────────────────────────

function renderSkillsBrowseTab() {
  const loading   = skillsState.loadingBrowse;
  const query     = skillsState.query;
  const canSearch = query.trim().length > 0;

  const content = loading
    ? renderInlineStatus('Searching skills...', true)
    : skillsState.browseRows.length > 0
      ? `
        <div class="settings-skills__list">
          ${skillsState.browseRows.map(row => {
            const actionKey       = `install:${skillIdentity(row.slug, row.version)}`;
            const isInstalling    = skillsState.actionKey === actionKey;
            const controlsDisabled = Boolean(skillsState.actionKey) && !isInstalling;
            return `
              <article class="settings-skills__item">
                <div class="settings-skills__item-main">
                  <div class="settings-skills__item-title">
                    <span>${escapeHtml(row.displayName)}</span>
                    <span class="settings-badge">${escapeHtml(row.slug)}</span>
                  </div>
                  <div class="settings-skills__item-meta">
                    v${escapeHtml(row.version)} | ${escapeHtml(row.registryName)}
                  </div>
                  <div class="settings-skills__item-summary">${escapeHtml(row.summary)}</div>
                </div>
                <div class="settings-skills__item-actions">
                  <button class="btn btn--primary btn--sm"
                          type="button"
                          data-skills-install="${escapeHtml(row.slug)}"
                          data-skills-install-version="${escapeHtml(row.version)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${isInstalling
                      ? '<span class="model-selector__spinner" aria-hidden="true"></span> Installing...'
                      : `${icon('plus', 14)} Install`}
                  </button>
                </div>
              </article>
            `;
          }).join('')}
        </div>
      `
      : renderInlineStatus(
          skillsState.lastQuery
            ? `No skills found for "${skillsState.lastQuery}".`
            : 'Search OHub skills by slug, name, or summary.'
        );

  return `
    <section class="settings-skills__section">
      <div class="settings-skills__toolbar">
        <div class="settings-skills__search">
          <span class="settings-skills__search-icon">${icon('search', 14)}</span>
          <input class="settings-skills__search-input"
                 type="text"
                 data-skills-search-input
                 value="${escapeHtml(query)}"
                 placeholder="Search skills (e.g. review, security, docs)">
        </div>
        <button class="btn btn--secondary btn--sm"
                type="button"
                data-skills-search
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

// ── Installed Tab ─────────────────────────────────────────────────────────────

function renderSkillsInstalledTab() {
  const loading = skillsState.loadingInstalled;
  const content = loading
    ? renderInlineStatus('Loading installed skills...', true)
    : skillsState.installedRows.length > 0
      ? `
        <div class="settings-skills__list">
          ${skillsState.installedRows.map(row => {
            const actionKey       = `uninstall:${row.slug}`;
            const isRemoving      = skillsState.actionKey === actionKey;
            const controlsDisabled = Boolean(skillsState.actionKey) && !isRemoving;
            return `
              <article class="settings-skills__item">
                <div class="settings-skills__item-main">
                  <div class="settings-skills__item-title">
                    <span>${escapeHtml(row.displayName)}</span>
                    <span class="settings-badge settings-badge--success">Installed</span>
                  </div>
                  <div class="settings-skills__item-meta">
                    ${escapeHtml(row.slug)} | v${escapeHtml(row.version)}
                  </div>
                  <div class="settings-skills__item-summary">${escapeHtml(row.summary)}</div>
                </div>
                <div class="settings-skills__item-actions">
                  <button class="btn btn--ghost btn--sm"
                          type="button"
                          data-skills-uninstall="${escapeHtml(row.slug)}"
                          ${controlsDisabled ? 'disabled' : ''}>
                    ${isRemoving
                      ? '<span class="model-selector__spinner" aria-hidden="true"></span> Removing...'
                      : `${icon('close', 14)} Uninstall`}
                  </button>
                </div>
              </article>
            `;
          }).join('')}
        </div>
      `
      : renderInlineStatus('No skills installed.');

  return `
    <section class="settings-skills__section">
      <div class="settings-skills__toolbar">
        <span class="settings-skills__toolbar-title">Installed skills in workspace</span>
        <button class="btn btn--ghost btn--sm"
                type="button"
                data-skills-refresh-installed
                ${loading ? 'disabled' : ''}>
          ${icon('refresh', 14)} Refresh
        </button>
      </div>
      ${content}
    </section>
  `;
}

// ── Event Binding ─────────────────────────────────────────────────────────────

function bindSkillsEvents(modal) {
  const host = modal?.querySelector('[data-skills-host]');
  if (!host) return;

  host.querySelectorAll('[data-skills-tab]').forEach(button => {
    button.addEventListener('click', () => {
      const nextTab = normalizeSkillTab(button.getAttribute('data-skills-tab'));
      if (nextTab === skillsState.activeTab) return;
      skillsState.activeTab = nextTab;
      renderSkillsStage(modal);
      if (nextTab === 'installed') void loadInstalledSkills(modal, false);
    });
  });

  const searchInput = host.querySelector('[data-skills-search-input]');
  if (searchInput instanceof HTMLInputElement) {
    searchInput.addEventListener('input', () => { skillsState.query = searchInput.value; });
    searchInput.addEventListener('keydown', event => {
      if (event.key === 'Enter') { event.preventDefault(); void searchSkillsMarketplace(modal); }
    });
  }

  host.querySelector('[data-skills-search]')?.addEventListener('click', () => {
    void searchSkillsMarketplace(modal);
  });

  host.querySelector('[data-skills-refresh-installed]')?.addEventListener('click', () => {
    void loadInstalledSkills(modal, true);
  });

  host.querySelectorAll('[data-skills-install]').forEach(button => {
    button.addEventListener('click', () => {
      const slug    = String(button.getAttribute('data-skills-install') || '').trim();
      const version = String(button.getAttribute('data-skills-install-version') || '').trim();
      if (!slug) return;
      void handleInstallSkill(modal, slug, version);
    });
  });

  host.querySelectorAll('[data-skills-uninstall]').forEach(button => {
    button.addEventListener('click', () => {
      const slug = String(button.getAttribute('data-skills-uninstall') || '').trim();
      if (!slug) return;
      void handleUninstallSkill(modal, slug);
    });
  });
}

// ── Async Handlers ────────────────────────────────────────────────────────────

async function searchSkillsMarketplace(modal, force = false) {
  if (skillsState.loadingBrowse) return;
  if (skillsState.actionKey && !force) return;

  const host  = modal?.querySelector('[data-skills-host]');
  const input = host?.querySelector('[data-skills-search-input]');
  if (input instanceof HTMLInputElement) skillsState.query = input.value;

  const query = String(skillsState.query || '').trim();
  if (!query) { showError('Search query cannot be empty.'); return; }

  if (!force && query === skillsState.lastQuery && skillsState.browseRows.length > 0) return;

  skillsState.loadingBrowse = true;
  skillsState.status = `Searching skills for "${query}"...`;
  renderSkillsStage(modal);

  try {
    // PLACEHOLDER: Simulate skill search with delay
    await new Promise(resolve => setTimeout(resolve, 300));
    const rows = PLACEHOLDER_SKILLS_SEARCH_RESULTS.filter(skill => 
      skill.slug.toLowerCase().includes(query.toLowerCase()) ||
      skill.displayName.toLowerCase().includes(query.toLowerCase()) ||
      skill.summary.toLowerCase().includes(query.toLowerCase())
    );
    skillsState.browseRows = Array.isArray(rows) ? rows.map(normalizeSkillSearchRow) : [];
    skillsState.lastQuery  = query;
    skillsState.status = skillsState.browseRows.length > 0
      ? `Found ${skillsState.browseRows.length} skills for "${query}".`
      : `No skills found for "${query}".`;
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to search skills.'));
  } finally {
    skillsState.loadingBrowse = false;
    renderSkillsStage(modal);
  }
}

async function loadInstalledSkills(modal, force = false) {
  if (skillsState.loadingInstalled) return;
  if (!force && skillsState.installedRows.length > 0) { renderSkillsStage(modal); return; }

  skillsState.loadingInstalled = true;
  renderSkillsStage(modal);

  try {
    // PLACEHOLDER: Load installed skills with delay
    await new Promise(resolve => setTimeout(resolve, 300));
    const rows = PLACEHOLDER_INSTALLED_SKILLS;
    skillsState.installedRows = Array.isArray(rows) ? rows.map(normalizeInstalledSkillRow) : [];
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to load installed skills.'));
  } finally {
    skillsState.loadingInstalled = false;
    renderSkillsStage(modal);
  }
}

async function withSkillAction(modal, actionKey, pendingStatus, errorMessage, action) {
  if (skillsState.actionKey) return null;
  skillsState.actionKey = actionKey;
  skillsState.status    = pendingStatus;
  renderSkillsStage(modal);
  try {
    return await action();
  } catch (error) {
    showError(normalizeErrorMessage(error, errorMessage));
    return null;
  } finally {
    skillsState.actionKey = '';
    renderSkillsStage(modal);
  }
}

async function handleInstallSkill(modal, slug, version) {
  const actionKey = `install:${skillIdentity(slug, version)}`;
  const payload   = await withSkillAction(
    modal, actionKey,
    `Installing skill ${slug}${version ? `@${version}` : ''}...`,
    'Failed to install skill.',
    async () => {
      // PLACEHOLDER: Simulate skill installation
      await new Promise(resolve => setTimeout(resolve, 400));
      return { message: `Installed skill '${slug}'`, slug, version: version || '1.0.0' };
    }
  );
  if (!payload) return;
  skillsState.status = String(payload?.message || `Installed skill '${slug}'`);
  // Add to installed list
  if (!skillsState.installedRows.find(r => r.slug === slug)) {
    const browseRow = skillsState.browseRows.find(r => r.slug === slug);
    if (browseRow) {
      skillsState.installedRows.push({
        slug: browseRow.slug,
        displayName: browseRow.displayName,
        summary: browseRow.summary,
        version: browseRow.version,
      });
    }
  }
  await loadInstalledSkills(modal, true);
}

async function handleUninstallSkill(modal, slug) {
  const actionKey = `uninstall:${slug}`;
  const payload   = await withSkillAction(
    modal, actionKey,
    `Uninstalling skill ${slug}...`,
    'Failed to uninstall skill.',
    async () => {
      // PLACEHOLDER: Simulate skill uninstallation
      await new Promise(resolve => setTimeout(resolve, 400));
      return { message: `Uninstalled skill '${slug}'` };
    }
  );
  if (!payload) return;
  skillsState.status = String(payload?.message || `Uninstalled skill '${slug}'`);
  // Remove from installed list
  skillsState.installedRows = skillsState.installedRows.filter(r => r.slug !== slug);
  await loadInstalledSkills(modal, true);
}

// ── Normalizers / Helpers ─────────────────────────────────────────────────────

function normalizeSkillTab(value) {
  return String(value || '').trim().toLowerCase() === 'installed' ? 'installed' : 'browse';
}

function normalizeSkillSearchRow(row) {
  return {
    slug:         String(row?.slug || '').trim(),
    displayName:  String(row?.displayName || '').trim() || 'Skill',
    summary:      String(row?.summary || '').trim() || 'No summary available.',
    version:      String(row?.version || '').trim() || '0.0.0',
    registryName: String(row?.registryName || '').trim() || 'registry',
  };
}

function normalizeInstalledSkillRow(row) {
  return {
    slug:        String(row?.slug || '').trim(),
    displayName: String(row?.displayName || '').trim() || 'Skill',
    summary:     String(row?.summary || '').trim() || 'No summary available.',
    version:     String(row?.version || '').trim() || '0.0.0',
  };
}

function skillIdentity(slug, version = '') {
  const normalizedSlug    = String(slug || '').trim().toLowerCase();
  const normalizedVersion = String(version || '').trim();
  return normalizedVersion ? `${normalizedSlug}@${normalizedVersion}` : normalizedSlug;
}

function escapeHtmlLocal(raw) {
  return String(raw ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { resetSkillsSettingsState, hydrateSkillsPage };
