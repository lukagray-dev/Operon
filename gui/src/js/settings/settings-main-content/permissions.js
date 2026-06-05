'use strict';

/**
 * permissions.js
 *
 * Permissions settings page for the Operon settings panel.
 *
 * Manages two top-level tabs:
 *  1. Directory tab — shows allowed directories list with per-directory configure view.
 *  2. Global tab    — shows global permission rows.
 *
 * Within the directory configure view, permission rows are grouped by their
 * parent group with collapse/expand support.
 *
 * All IPC calls go through the shared ipc module.
 * Transient UI state is held in permissionsState (local, not in the store).
 */

import { showError, showSuccess } from '../../shared/toast.js';
import {
  activeCategory,
  renderInlineStatus,
  escapeHtml,
  escapeAttribute,
  normalizeErrorMessage,
} from '../settings-panel.js';
import * as IPC from '../../shared/ipc.js';

// ── Transient Module State ────────────────────────────────────────────────────

const permissionsState = {
  activeTab: 'directory',
  scope: 'owner',
  workspaceDirectory: '',
  directories: [],
  selectedDirectory: '',
  configureDirectory: '',
  activeView: 'list',
  rows: [],
  loadingDirectories: false,
  loadingRows: false,
  savingRowKey: '',
  status: '',
  expandedGroups: new Set(),
};

// ── Icon Helper ───────────────────────────────────────────────────────────────

/**
 * Returns an <img> tag for permission icons from the settings icons directory.
 * @param {string} name - Icon name without extension
 * @param {number} size - Icon size in pixels
 * @returns {string} HTML img tag
 */
function permissionIcon(name, size = 16) {
  return `<img src="./assets/icons/settings/${name}.svg" width="${size}" height="${size}" alt="" draggable="false">`;
}

// ── State Reset ───────────────────────────────────────────────────────────────

function resetPermissionsSettingsState() {
  permissionsState.activeTab = 'directory';
  permissionsState.scope = 'owner';
  permissionsState.workspaceDirectory = '';
  permissionsState.directories = [];
  permissionsState.selectedDirectory = '';
  permissionsState.rows = [];
  permissionsState.loadingDirectories = false;
  permissionsState.loadingRows = false;
  permissionsState.savingRowKey = '';
  permissionsState.status = '';
  permissionsState.activeView = 'list';
  permissionsState.configureDirectory = '';
}

// ── Hydration Entry Point ─────────────────────────────────────────────────────

async function hydratePermissionsPage(modal) {
  if (!modal || activeCategory !== 'permissions') return;

  if (permissionsState.directories.length === 0 && !permissionsState.loadingDirectories) {
    await refreshAllowedDirectories(modal, false);
  }

  if (permissionsState.activeTab === 'directory' && !permissionsState.selectedDirectory) {
    permissionsState.selectedDirectory =
      permissionsState.workspaceDirectory ||
      permissionsState.directories[0] ||
      '';
  }

  renderPermissionsStage(modal);
  await loadPermissionRows(modal);
}

// ── Stage Renderer ────────────────────────────────────────────────────────────

async function refreshAllowedDirectories(modal, preserveSelection = true) {
  permissionsState.loadingDirectories = true;
  renderPermissionsStage(modal);

  try {
    const payload   = await IPC.getAllowedDirectories();
    const normalized = normalizeAllowedDirectoriesPayload(payload);
    permissionsState.workspaceDirectory = normalized.workspaceDirectory;
    permissionsState.directories = normalized.directories;

    if (permissionsState.directories.length === 0 && normalized.workspaceDirectory) {
      permissionsState.directories = [normalized.workspaceDirectory];
    }

    if (!preserveSelection || !permissionsState.directories.includes(permissionsState.selectedDirectory)) {
      permissionsState.selectedDirectory =
        normalized.workspaceDirectory ||
        permissionsState.directories[0] ||
        '';
    }
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to load allowed directories.'));
  } finally {
    permissionsState.loadingDirectories = false;
    renderPermissionsStage(modal);
  }
}

async function loadPermissionRows(modal) {
  if (!modal || activeCategory !== 'permissions') return;

  permissionsState.loadingRows = true;
  renderPermissionsStage(modal);

  try {
    let rows = [];
    if (permissionsState.activeTab === 'global') {
      rows = await IPC.getPermissionRows(permissionsState.scope);
    } else if (permissionsState.configureDirectory) {
      rows = await IPC.getPermissionRows(permissionsState.scope, permissionsState.configureDirectory);
    }

    permissionsState.rows = Array.isArray(rows) ? rows.map(normalizePermissionRow) : [];
    permissionsState.status = '';
  } catch (error) {
    permissionsState.rows = [];
    showError(normalizeErrorMessage(error, 'Failed to load permission rows.'));
  } finally {
    permissionsState.loadingRows = false;
    renderPermissionsStage(modal);
  }
}

function renderPermissionsStage(modal) {
  const host = modal?.querySelector('[data-permissions-host]');
  if (!host) return;

  const isDirectoryTab = permissionsState.activeTab === 'directory';

  if (isDirectoryTab && permissionsState.activeView === 'configure' && permissionsState.configureDirectory) {
    host.innerHTML = renderToolsPermissionsConfigureView();
  } else if (isDirectoryTab) {
    host.innerHTML = renderAllowedDirectoriesList();
  } else {
    host.innerHTML = renderGlobalPermissionsList();
  }

  bindPermissionsEvents(modal);
}

// ── Views ─────────────────────────────────────────────────────────────────────

function renderAllowedDirectoriesList() {
  const isDirectoryTab = permissionsState.activeTab === 'directory';

  const directoryItems = permissionsState.directories.map(directory => {
    const isWorkspace = directory === permissionsState.workspaceDirectory;
    return `
      <div class="settings-permissions__directory-row">
        <div class="settings-permissions__directory-info">
          <span class="settings-permissions__directory-icon">${permissionIcon('permission-directory', 16)}</span>
          <span class="settings-permissions__directory-path">${escapeHtml(directory)}</span>
          ${isWorkspace ? '<span class="settings-badge settings-badge--success">Workspace</span>' : ''}
        </div>
        <div class="settings-permissions__directory-actions">
          <button class="btn btn--ghost btn--sm"
                  type="button"
                  data-permissions-configure-directory="${escapeAttribute(directory)}"
                  title="Configure permissions">
            ${permissionIcon('configure-directory-permissions', 14)}
          </button>
          ${!isWorkspace ? `
            <button class="btn btn--ghost btn--sm"
                    type="button"
                    data-permissions-remove-directory="${escapeAttribute(directory)}"
                    title="Remove directory">
              ${permissionIcon('permission-directory-remove', 14)}
            </button>
          ` : ''}
        </div>
      </div>
    `;
  }).join('');

  return `
    <div class="settings-permissions__tabs settings-tabs">
      <button class="settings-tabs__item ${isDirectoryTab ? 'is-active' : ''}"
              type="button"
              data-permissions-tab="directory">
        Allowed Directories
      </button>
      <button class="settings-tabs__item ${!isDirectoryTab ? 'is-active' : ''}"
              type="button"
              data-permissions-tab="global">
        Global
      </button>
    </div>

    ${permissionsState.loadingDirectories
      ? renderInlineStatus('Loading directories...', true)
      : (directoryItems || renderInlineStatus('No directories configured.'))}

    <div class="settings-row" style="margin-top: 12px; padding-top: 12px; border-top: 1px solid #1e1e1e;">
      <div class="settings-row__info">
        <div class="settings-row__label">Add directory</div>
        <div class="settings-row__description">Allow tools to access a new directory</div>
      </div>
      <div class="settings-row__control" style="display: flex; gap: 8px;">
        <input class="settings-input"
               type="text"
               data-permissions-directory-input
               placeholder="Directory path">
        <button class="btn btn--secondary btn--sm"
                type="button"
                data-permissions-add-directory
                ${permissionsState.loadingDirectories ? 'disabled' : ''}>
          ${permissionIcon('plus', 14)}
        </button>
      </div>
    </div>
  `;
}

function renderToolsPermissionsConfigureView() {
  const directory  = permissionsState.configureDirectory;
  const scopeLabel = permissionsState.scope === 'external' ? 'external' : 'owner';

  const topbar = `
    <div class="settings-models__topbar">
      <button class="btn btn--ghost btn--sm" type="button" data-permissions-back>
        ${permissionIcon('chevron-down', 14)} Back
      </button>
    </div>
  `;

  const scopeTabs = `
    <div class="settings-permissions__toolbar">
      <div class="settings-permissions__scope" role="group" aria-label="Permission scope">
        <button class="settings-permissions__scope-btn ${permissionsState.scope === 'owner' ? 'is-active' : ''}"
                type="button" data-permissions-scope="owner">
          Owner
        </button>
        <button class="settings-permissions__scope-btn ${permissionsState.scope === 'external' ? 'is-active' : ''}"
                type="button" data-permissions-scope="external">
          External
        </button>
      </div>
      <div class="settings-permissions__summary">${escapeHtml(scopeLabel)} scope</div>
    </div>
  `;

  const content = renderGroupedPermissionRows();

  return `
    <div class="settings-permissions__view--configure">
      ${topbar}
      ${scopeTabs}
      ${content}
      ${permissionsState.status ? `<div class="settings-permissions__status">${escapeHtml(permissionsState.status)}</div>` : ''}
    </div>
  `;
}

function renderGlobalPermissionsList() {
  const scopeLabel     = permissionsState.scope === 'external' ? 'external' : 'owner';
  const isDirectoryTab = permissionsState.activeTab === 'directory';

  const scopeTabs = `
    <div class="settings-permissions__toolbar">
      <div class="settings-permissions__scope" role="group" aria-label="Permission scope">
        <button class="settings-permissions__scope-btn ${permissionsState.scope === 'owner' ? 'is-active' : ''}"
                type="button" data-permissions-scope="owner">Owner</button>
        <button class="settings-permissions__scope-btn ${permissionsState.scope === 'external' ? 'is-active' : ''}"
                type="button" data-permissions-scope="external">External</button>
      </div>
      <div class="settings-permissions__summary">Global · ${escapeHtml(scopeLabel)} scope</div>
    </div>
  `;

  return `
    <div class="settings-permissions__tabs settings-tabs">
      <button class="settings-tabs__item ${isDirectoryTab ? 'is-active' : ''}"
              type="button" data-permissions-tab="directory">
        Allowed Directories
      </button>
      <button class="settings-tabs__item ${!isDirectoryTab ? 'is-active' : ''}"
              type="button" data-permissions-tab="global">
        Global
      </button>
    </div>

    ${scopeTabs}
    ${renderGroupedPermissionRows()}

    ${permissionsState.status ? `<div class="settings-permissions__status">${escapeHtml(permissionsState.status)}</div>` : ''}
  `;
}

// ── Grouped Permission Row Renderer ───────────────────────────────────────────

function renderGroupedPermissionRows() {
  if (permissionsState.loadingRows) {
    return renderInlineStatus('Loading permissions...', true);
  }

  if (permissionsState.rows.length === 0) {
    return renderInlineStatus('No permission rows.');
  }

  const groups = new Map();
  const tools  = [];

  permissionsState.rows.forEach(row => {
    if (row.kind === 'group') {
      groups.set(row.key, { group: row, tools: [] });
    } else {
      tools.push(row);
    }
  });

  tools.forEach(tool => {
    const gk = tool.groupKey;
    if (gk && groups.has(gk)) {
      groups.get(gk).tools.push(tool);
    }
  });

  const groupItems = [];
  groups.forEach(({ group, tools: toolList }) => {
    const isExpanded = permissionsState.expandedGroups.has(group.key);
    groupItems.push(renderPermissionGroupHeader(group, toolList.length > 0, isExpanded));
    if (isExpanded && toolList.length > 0) {
      toolList.forEach(tool => groupItems.push(renderPermissionToolRow(tool)));
    }
  });

  return groupItems.join('');
}

function buildPermissionModeToggle(row) {
  const values = ['allow', 'ask', 'deny'];
  const icons  = { allow: 'permission-allow', ask: 'permission-ask', deny: 'permission-deny' };
  const titles = { allow: 'Allow', ask: 'Ask', deny: 'Deny' };
  const activeMode = row.mode === 'custom' ? '' : row.mode;
  const rowKind    = row.kind === 'group' ? 'group' : 'tool';

  return `
    <div class="permission-toggle">
      ${values.map(value => `
        <button class="permission-toggle__btn ${value === activeMode ? 'is-active' : ''}"
                data-permissions-set-mode
                data-permissions-mode="${value}"
                data-permissions-row-kind="${rowKind}"
                data-permissions-permission-key="${escapeAttribute(row.key)}"
                title="${titles[value]}">
          ${permissionIcon(icons[value], 14)}
        </button>
      `).join('')}
    </div>
  `;
}

function renderPermissionGroupHeader(group, hasTools, isExpanded) {
  const rowId     = permissionRowIdentifier(group);
  const isSaving  = permissionsState.savingRowKey === rowId;
  const modeLabel = group.mode === 'custom' ? 'custom' : group.mode;
  const modeMeta  = group.isExplicit ? 'explicit' : `inherits ${group.baseMode}`;

  return `
    <div class="settings-permissions__permission-row settings-permissions__permission-row--group">
      <div class="settings-permissions__permission-meta">
        <div class="settings-permissions__permission-title settings-permissions__permission-title--group">
          <span class="settings-permissions__permission-icon">${permissionIcon('package', 16)}</span>
          <span>${escapeHtml(group.label)}</span>
          ${group.mode === 'custom' ? '<span class="settings-badge">Custom</span>' : ''}
        </div>
        <div class="settings-permissions__permission-subtitle">
          ${escapeHtml(group.kind)} · key: ${escapeHtml(group.key)} · ${escapeHtml(modeMeta)}
        </div>
      </div>
      <div class="settings-permissions__permission-controls">
        ${buildPermissionModeToggle(group)}
        <span class="settings-permissions__permission-mode">${escapeHtml(modeLabel)}</span>
        ${isSaving ? '<span class="model-selector__spinner" aria-hidden="true"></span>' : ''}
        ${hasTools ? `
          <button class="btn btn--ghost btn--sm settings-permissions__expand-btn"
                  type="button"
                  data-permissions-toggle-group="${escapeAttribute(group.key)}"
                  title="${isExpanded ? 'Collapse' : 'Expand'} tools">
            ${isExpanded ? permissionIcon('chevron-up', 14) : permissionIcon('chevron-down', 14)}
          </button>
        ` : ''}
      </div>
    </div>
  `;
}

function renderPermissionToolRow(tool) {
  const rowId     = permissionRowIdentifier(tool);
  const isSaving  = permissionsState.savingRowKey === rowId;
  const modeLabel = tool.mode === 'custom' ? 'custom' : tool.mode;
  const modeMeta  = tool.isExplicit ? 'explicit' : `inherits ${tool.baseMode}`;

  return `
    <div class="settings-permissions__permission-row settings-permissions__permission-row--tool">
      <div class="settings-permissions__permission-meta">
        <div class="settings-permissions__permission-title">
          <span class="settings-permissions__permission-icon">${permissionIcon('code', 12)}</span>
          <span>${escapeHtml(tool.label)}</span>
          ${tool.mode === 'custom' ? '<span class="settings-badge">Custom</span>' : ''}
        </div>
        <div class="settings-permissions__permission-subtitle">
          ${escapeHtml(tool.kind)} · key: ${escapeHtml(tool.key)} · ${escapeHtml(modeMeta)}
        </div>
      </div>
      <div class="settings-permissions__permission-controls">
        ${buildPermissionModeToggle(tool)}
        <span class="settings-permissions__permission-mode">${escapeHtml(modeLabel)}</span>
        ${isSaving ? '<span class="model-selector__spinner" aria-hidden="true"></span>' : ''}
      </div>
    </div>
  `;
}

// ── Event Binding ─────────────────────────────────────────────────────────────

function bindPermissionsEvents(modal) {
  const host = modal?.querySelector('[data-permissions-host]');
  if (!host) return;

  host.querySelectorAll('[data-permissions-tab]').forEach(button => {
    button.addEventListener('click', () => {
      const tab = normalizePermissionTab(button.getAttribute('data-permissions-tab'));
      if (permissionsState.activeTab === tab) return;
      permissionsState.activeTab = tab;
      permissionsState.rows = [];
      permissionsState.status = '';
      void loadPermissionRows(modal);
    });
  });

  host.querySelectorAll('[data-permissions-scope]').forEach(button => {
    button.addEventListener('click', () => {
      const scope = normalizePermissionScope(button.getAttribute('data-permissions-scope'));
      if (permissionsState.scope === scope) return;
      permissionsState.scope = scope;
      permissionsState.rows = [];
      permissionsState.status = '';
      void loadPermissionRows(modal);
    });
  });

  host.querySelectorAll('[data-permissions-configure-directory]').forEach(button => {
    button.addEventListener('click', () => {
      const directory = String(button.getAttribute('data-permissions-configure-directory') || '').trim();
      if (!directory) return;
      permissionsState.configureDirectory = directory;
      permissionsState.activeView = 'configure';
      permissionsState.rows = [];
      permissionsState.status = '';
      void loadPermissionRows(modal);
    });
  });

  host.querySelectorAll('[data-permissions-back]').forEach(button => {
    button.addEventListener('click', () => {
      permissionsState.activeView = 'list';
      permissionsState.configureDirectory = '';
      permissionsState.rows = [];
      permissionsState.status = '';
      renderPermissionsStage(modal);
    });
  });

  host.querySelectorAll('[data-permissions-remove-directory]').forEach(button => {
    button.addEventListener('click', () => {
      const directory = String(button.getAttribute('data-permissions-remove-directory') || '').trim();
      if (!directory) return;
      void handleRemoveAllowedDirectory(modal, directory);
    });
  });

  const addButton = host.querySelector('[data-permissions-add-directory]');
  const addInput  = host.querySelector('[data-permissions-directory-input]');
  addButton?.addEventListener('click', () => void handleAddAllowedDirectory(modal));
  addInput?.addEventListener('keydown', event => {
    if (event.key === 'Enter') {
      event.preventDefault();
      void handleAddAllowedDirectory(modal);
    }
  });

  host.querySelectorAll('[data-permissions-set-mode]').forEach(button => {
    button.addEventListener('click', () => {
      const rowKind       = String(button.getAttribute('data-permissions-row-kind') || '').trim();
      const permissionKey = String(button.getAttribute('data-permissions-permission-key') || '').trim();
      const mode          = String(button.getAttribute('data-permissions-mode') || '').trim();
      if (!rowKind || !permissionKey || !mode) return;
      void applyPermissionModeUpdate(modal, rowKind, permissionKey, mode);
    });
  });

  host.querySelectorAll('[data-permissions-toggle-group]').forEach(button => {
    button.addEventListener('click', () => {
      const groupKey = String(button.getAttribute('data-permissions-toggle-group') || '').trim();
      if (!groupKey) return;
      if (permissionsState.expandedGroups.has(groupKey)) {
        permissionsState.expandedGroups.delete(groupKey);
      } else {
        permissionsState.expandedGroups.add(groupKey);
      }
      renderPermissionsStage(modal);
    });
  });
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async function handleAddAllowedDirectory(modal) {
  const host  = modal?.querySelector('[data-permissions-host]');
  const input = host?.querySelector('[data-permissions-directory-input]');
  if (!(input instanceof HTMLInputElement)) return;

  const directory = input.value.trim();
  if (!directory) { showError('Directory cannot be empty.'); return; }

  permissionsState.loadingDirectories = true;
  permissionsState.status = 'Adding allowed directory...';
  renderPermissionsStage(modal);

  try {
    const payload = await IPC.addAllowedDirectory(directory);
    const normalized = normalizeAllowedDirectoriesPayload(payload);
    permissionsState.workspaceDirectory = normalized.workspaceDirectory;
    permissionsState.directories = normalized.directories;
    permissionsState.selectedDirectory = directory;
    permissionsState.status = `Added allowed directory: ${directory}`;
    input.value = '';
    showSuccess(`Added directory: ${directory}`);
    await loadPermissionRows(modal);
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to add allowed directory.'));
  } finally {
    permissionsState.loadingDirectories = false;
    renderPermissionsStage(modal);
  }
}

async function handleRemoveAllowedDirectory(modal, directory) {
  permissionsState.loadingDirectories = true;
  permissionsState.status = `Removing allowed directory: ${directory}...`;
  renderPermissionsStage(modal);

  try {
    const payload = await IPC.removeAllowedDirectory(directory);
    const normalized = normalizeAllowedDirectoriesPayload(payload);
    permissionsState.workspaceDirectory = normalized.workspaceDirectory;
    permissionsState.directories = normalized.directories;

    if (!permissionsState.directories.includes(permissionsState.selectedDirectory)) {
      permissionsState.selectedDirectory =
        normalized.workspaceDirectory || permissionsState.directories[0] || '';
    }

    permissionsState.status = `Removed allowed directory: ${directory}`;
    showSuccess(`Removed directory: ${directory}`);
    await loadPermissionRows(modal);
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to remove allowed directory.'));
  } finally {
    permissionsState.loadingDirectories = false;
    renderPermissionsStage(modal);
  }
}

async function applyPermissionModeUpdate(modal, rowKind, permissionKey, targetMode) {
  if (permissionsState.loadingRows || permissionsState.savingRowKey) return;

  const row = permissionsState.rows.find(r => r.kind === rowKind && r.key === permissionKey);
  if (!row) return;

  const target = normalizePermissionMode(targetMode);
  if (!target) return;

  // Selecting the inherited/base mode clears explicit override
  const modeToSave = target === row.baseMode ? null : target;

  permissionsState.savingRowKey = permissionRowIdentifier(row);
  permissionsState.status = `Applying ${rowKind} permission update...`;
  renderPermissionsStage(modal);

  try {
    const isGlobal = permissionsState.activeTab === 'global';
    const directory = isGlobal ? null : permissionsState.configureDirectory;

    await IPC.updatePermissionMode(
      permissionsState.scope,
      directory,
      permissionKey,
      modeToSave
    );
    
    // Refresh rows from backend to ensure consistent state and reflect any cascade/inheritance correctly
    await loadPermissionRows(modal);
    
    permissionsState.status = `${row.label} updated`;
    showSuccess(`Updated ${row.label} to ${target}`);
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to update permission mode.'));
  } finally {
    permissionsState.savingRowKey = '';
    renderPermissionsStage(modal);
  }
}

// ── Normalizers ───────────────────────────────────────────────────────────────

function normalizeAllowedDirectoriesPayload(payload) {
  const workspaceDirectory = String(payload?.workspaceDirectory || '').trim();
  const directories = Array.isArray(payload?.directories)
    ? payload.directories.map(e => String(e || '').trim()).filter(Boolean)
    : [];

  if (workspaceDirectory && !directories.includes(workspaceDirectory)) {
    directories.unshift(workspaceDirectory);
  }

  return { workspaceDirectory, directories };
}

function normalizePermissionRow(row) {
  const kind = row?.kind === 'tool' ? 'tool' : 'group';
  return {
    key:        String(row?.key || '').trim(),
    label:      String(row?.label || '').trim() || 'Permission',
    mode:       normalizePermissionMode(row?.mode) || 'ask',
    baseMode:   normalizePermissionMode(row?.baseMode) || 'ask',
    isExplicit: Boolean(row?.isExplicit),
    kind,
    groupKey:   String(row?.groupKey || '').trim(),
  };
}

function normalizePermissionMode(value) {
  const normalized = String(value || '').trim().toLowerCase();
  if (['allow', 'ask', 'deny', 'custom'].includes(normalized)) return normalized;
  return '';
}

function normalizePermissionScope(value) {
  return String(value || '').trim().toLowerCase() === 'external' ? 'external' : 'owner';
}

function normalizePermissionTab(value) {
  return String(value || '').trim().toLowerCase() === 'global' ? 'global' : 'directory';
}

function permissionRowIdentifier(row) {
  return `${row.kind}:${row.key}`;
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { resetPermissionsSettingsState, hydratePermissionsPage };
