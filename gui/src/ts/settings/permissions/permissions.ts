// Permissions Settings Controller & DOM Coordinator
//
// 1:1 implementation matching Slint permissions.slint and permissions.rs:
// - Top Tab switcher: "Allowed Directories" vs "Global Permissions"
// - Allowed Directories view: directory rows with workspace badge, configure button, delete button, and Add Directory input with native folder picker
// - Directory Configuration view: back button, directory path header, scope selector, group/tool permission rows
// - Global Permissions view: scope selector (Owner vs External), expandable group rows, tool rows, and segmented Allow / Ask / Deny mode buttons

import {
  addAllowedDirectoryIpc,
  getAllowedDirectoriesIpc,
  getPermissionItemsIpc,
  pickAllowedDirectoryDialogIpc,
  removeAllowedDirectoryIpc,
  updatePermissionModeIpc,
} from './ipc.js';
import type { AllowedDirectories, PermissionItem } from './types.js';

let activeTab = 0; // 0 = Allowed Directories, 1 = Global Permissions
let activeScope: 'owner' | 'external' = 'owner';
let configureDirectory = '';
const expandedGroups: Set<string> = new Set();
let allowedDirsData: AllowedDirectories = { directories: [], workspace_directory: '' };
let currentPermissionItems: PermissionItem[] = [];

/**
 * Initializes the Permissions Settings panel.
 */
export async function initPermissionsSettings(): Promise<void> {
  setupTabSelectors();
  setupScopeSelectors();
  setupAddDirectoryActions();
  setupDirectoryBackAction();
  await refreshPermissionsData();
}

/**
 * Refreshes both allowed directories and permission rows from the backend.
 */
export async function refreshPermissionsData(): Promise<void> {
  try {
    allowedDirsData = await getAllowedDirectoriesIpc();
    renderAllowedDirectories();

    if (activeTab === 1 || configureDirectory !== '') {
      await refreshPermissionRows();
    }
  } catch (err) {
    console.error('[PermissionsSettings] Failed to refresh permissions:', err);
  }
}

/**
 * Re-queries permission rows for current scope and optional configure directory.
 */
async function refreshPermissionRows(): Promise<void> {
  try {
    currentPermissionItems = await getPermissionItemsIpc(
      activeScope,
      configureDirectory || undefined
    );
    renderPermissionItems();
  } catch (err) {
    console.error('[PermissionsSettings] Failed to load permission items:', err);
  }
}

/**
 * Binds tab selection buttons (Allowed Directories vs Global Permissions).
 */
function setupTabSelectors(): void {
  const tabButtons = document.querySelectorAll<HTMLButtonElement>('.seg-choice-perm-tab');
  tabButtons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      activeTab = parseInt(btn.dataset.index || '0', 10);
      configureDirectory = '';
      tabButtons.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');
      updatePermissionsViewSwitch();
      await refreshPermissionsData();
    });
  });
}

/**
 * Binds scope selector buttons (Owner vs External).
 */
function setupScopeSelectors(): void {
  const scopeButtons = document.querySelectorAll<HTMLButtonElement>('.seg-choice-perm-scope');
  scopeButtons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      const idx = parseInt(btn.dataset.index || '0', 10);
      activeScope = idx === 0 ? 'owner' : 'external';
      scopeButtons.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');

      // Update scope label texts
      const scopeLabels = document.querySelectorAll<HTMLElement>('.perm-scope-info-label');
      scopeLabels.forEach((lbl) => {
        lbl.textContent =
          configureDirectory !== ''
            ? `Directory • ${activeScope === 'owner' ? 'Owner' : 'External'} scope`
            : `Global • ${activeScope === 'owner' ? 'Owner' : 'External'} scope`;
      });

      await refreshPermissionRows();
    });
  });
}

/**
 * Sets up the Add Directory input and native folder picker.
 */
function setupAddDirectoryActions(): void {
  const input = document.getElementById('input-perm-new-dir') as HTMLInputElement | null;
  const addBtn = document.getElementById('btn-perm-add-dir');
  const browseBtn = document.getElementById('btn-perm-browse-dir');

  addBtn?.addEventListener('click', async () => {
    if (!input) return;
    const path = input.value.trim();
    if (path) {
      await addAllowedDirectoryIpc(path);
      input.value = '';
      await refreshPermissionsData();
    }
  });

  browseBtn?.addEventListener('click', async () => {
    const picked = await pickAllowedDirectoryDialogIpc();
    if (picked) {
      if (input) {
        input.value = picked;
      }
      await addAllowedDirectoryIpc(picked);
      if (input) input.value = '';
      await refreshPermissionsData();
    }
  });
}

/**
 * Sets up back button from directory configuration view.
 */
function setupDirectoryBackAction(): void {
  document.getElementById('btn-perm-dir-back')?.addEventListener('click', async () => {
    configureDirectory = '';
    activeTab = 0;
    updatePermissionsViewSwitch();
    await refreshPermissionsData();
  });
}

/**
 * Renders the list of Allowed Directories.
 */
function renderAllowedDirectories(): void {
  const container = document.getElementById('perm-directories-container');
  if (!container) return;

  container.innerHTML = '';

  if (allowedDirsData.directories.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'perm-empty-row';
    empty.textContent = 'No allowed directories configured.';
    container.appendChild(empty);
    return;
  }

  allowedDirsData.directories.forEach((dirPath) => {
    const isWorkspace = dirPath === allowedDirsData.workspace_directory;
    const row = document.createElement('div');
    row.className = 'perm-dir-row';

    row.innerHTML = `
      <div class="perm-dir-left">
        <span class="ui-icon icon-perm-directory"></span>
        <span class="perm-dir-path" title="${dirPath}">${dirPath}</span>
        ${isWorkspace ? '<span class="perm-dir-workspace-badge">Workspace</span>' : ''}
      </div>
      <div class="perm-dir-actions">
        <button class="perm-dir-action-btn btn-configure-dir" title="Configure Tool Permissions">
          <span class="ui-icon icon-perm-configure"></span>
        </button>
        ${
          !isWorkspace
            ? `<button class="perm-dir-action-btn btn-remove-dir" title="Remove Allowed Directory">
                <span class="ui-icon icon-perm-delete"></span>
              </button>`
            : ''
        }
      </div>
    `;

    // Configure click
    row.querySelector('.btn-configure-dir')?.addEventListener('click', async () => {
      configureDirectory = dirPath;
      expandedGroups.clear();
      updatePermissionsViewSwitch();
      await refreshPermissionRows();
    });

    // Remove click
    row.querySelector('.btn-remove-dir')?.addEventListener('click', async () => {
      await removeAllowedDirectoryIpc(dirPath);
      await refreshPermissionsData();
    });

    container.appendChild(row);
  });
}

/**
 * Renders the group and tool permission rows with Allow/Ask/Deny selectors.
 */
function renderPermissionItems(): void {
  const container =
    configureDirectory !== ''
      ? document.getElementById('perm-dir-items-container')
      : document.getElementById('perm-global-items-container');

  if (!container) return;

  container.innerHTML = '';

  // Separate groups and tools
  const groups = currentPermissionItems.filter((item) => item.kind === 'group');
  const tools = currentPermissionItems.filter((item) => item.kind === 'tool');

  groups.forEach((g) => {
    const hasTools = tools.some((t) => t.group_key === g.key);
    const isExpanded = expandedGroups.has(g.key);

    const groupRow = document.createElement('div');
    groupRow.className = 'perm-item-row perm-group-row';

    groupRow.innerHTML = `
      <div class="perm-item-left">
        ${
          hasTools
            ? `<button class="perm-group-expand-btn ${isExpanded ? 'expanded' : ''}" title="Expand tools">
                <span class="ui-icon ${isExpanded ? 'icon-chevron-down' : 'icon-chevron-right'}"></span>
              </button>`
            : '<span class="perm-group-expand-spacer"></span>'
        }
        <div class="perm-item-info">
          <div class="perm-item-label">${g.label}</div>
          <div class="perm-item-subtitle">${g.subtitle}</div>
        </div>
      </div>
      <div class="perm-item-mode-control">
        <div class="perm-segmented-mode" data-key="${g.key}" data-kind="group">
          <button class="perm-mode-btn ${g.mode === 'allow' ? 'active allow' : ''}" data-mode="allow">Allow</button>
          <button class="perm-mode-btn ${g.mode === 'ask' ? 'active ask' : ''}" data-mode="ask">Ask</button>
          <button class="perm-mode-btn ${g.mode === 'deny' ? 'active deny' : ''}" data-mode="deny">Deny</button>
        </div>
      </div>
    `;

    // Expand toggle click
    groupRow.querySelector('.perm-group-expand-btn')?.addEventListener('click', () => {
      if (expandedGroups.has(g.key)) {
        expandedGroups.delete(g.key);
      } else {
        expandedGroups.add(g.key);
      }
      renderPermissionItems();
    });

    // Mode button clicks
    bindModeButtons(groupRow, g.key, 'group');

    container.appendChild(groupRow);

    // If expanded, render matching tools
    if (isExpanded) {
      tools
        .filter((t) => t.group_key === g.key)
        .forEach((t) => {
          const toolRow = document.createElement('div');
          toolRow.className = 'perm-item-row perm-tool-row';

          toolRow.innerHTML = `
            <div class="perm-item-left" style="padding-left: 28px;">
              <div class="perm-item-info">
                <div class="perm-item-label">${t.label}</div>
                <div class="perm-item-subtitle">${t.subtitle}</div>
              </div>
            </div>
            <div class="perm-item-mode-control">
              <div class="perm-segmented-mode" data-key="${t.key}" data-kind="tool">
                <button class="perm-mode-btn ${t.mode === 'allow' ? 'active allow' : ''}" data-mode="allow">Allow</button>
                <button class="perm-mode-btn ${t.mode === 'ask' ? 'active ask' : ''}" data-mode="ask">Ask</button>
                <button class="perm-mode-btn ${t.mode === 'deny' ? 'active deny' : ''}" data-mode="deny">Deny</button>
              </div>
            </div>
          `;

          bindModeButtons(toolRow, t.key, 'tool');
          container.appendChild(toolRow);
        });
    }
  });
}

/**
 * Binds Allow, Ask, Deny segmented click handlers.
 */
function bindModeButtons(rowEl: HTMLElement, key: string, kind: string): void {
  const modeButtons = rowEl.querySelectorAll<HTMLButtonElement>('.perm-mode-btn');
  modeButtons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      const mode = btn.dataset.mode || 'ask';
      await updatePermissionModeIpc({
        scope: activeScope,
        directory: configureDirectory || undefined,
        key,
        kind,
        mode,
      });
      await refreshPermissionRows();
    });
  });
}

/**
 * Updates UI view container visibility between Allowed Directories, Directory Configure, and Global Permissions.
 */
function updatePermissionsViewSwitch(): void {
  const mainTabsContainer = document.getElementById('perm-main-tabs-container');
  const allowedDirsView = document.getElementById('perm-view-allowed-dirs');
  const globalPermsView = document.getElementById('perm-view-global-perms');
  const dirConfigView = document.getElementById('perm-view-dir-config');
  const dirConfigPathTitle = document.getElementById('perm-dir-config-path-title');

  if (configureDirectory !== '') {
    // In directory configuration sub-view
    mainTabsContainer?.classList.add('hidden');
    allowedDirsView?.classList.add('hidden');
    globalPermsView?.classList.add('hidden');
    dirConfigView?.classList.remove('hidden');
    if (dirConfigPathTitle) {
      dirConfigPathTitle.textContent = `Directory: ${configureDirectory}`;
    }
  } else {
    // In main tabs view
    mainTabsContainer?.classList.remove('hidden');
    dirConfigView?.classList.add('hidden');

    if (activeTab === 0) {
      allowedDirsView?.classList.remove('hidden');
      globalPermsView?.classList.add('hidden');
    } else {
      allowedDirsView?.classList.add('hidden');
      globalPermsView?.classList.remove('hidden');
    }
  }
}
