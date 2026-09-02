// Feishu / Lark Channel Configuration View Controller

import {
  checkFeishuPolicyCoverageIpc,
  getFeishuStateIpc,
  pickFeishuWorkspaceDialogIpc,
  saveFeishuChannelConfigIpc,
  testFeishuChannelConnectionIpc,
} from './ipc.js';
import type { SaveFeishuPayload } from './types.js';

let allowlistTags: string[] = [];
let policyDebounceTimer: number | undefined;

export async function initFeishuView(): Promise<void> {
  const state = await getFeishuStateIpc();

  const domainSelect = document.getElementById('input-fs-domain') as HTMLSelectElement | null;
  const appIdInput = document.getElementById('input-fs-app-id') as HTMLInputElement | null;
  const appSecretInput = document.getElementById('input-fs-app-secret') as HTMLInputElement | null;
  const ownerInput = document.getElementById('input-fs-owner-user-id') as HTMLInputElement | null;
  const wsInput = document.getElementById('input-fs-workspace-dir') as HTMLInputElement | null;
  const statusBadge = document.getElementById('fs-connection-badge');
  const policyWarning = document.getElementById('fs-policy-warning');
  const statusMsg = document.getElementById('fs-test-status-msg');

  if (domainSelect) {
    domainSelect.value = state.domain || 'feishu';
  }
  if (appIdInput) {
    appIdInput.value = state.app_id;
  }
  if (appSecretInput) {
    appSecretInput.value = state.app_secret;
  }
  if (ownerInput) {
    ownerInput.value = state.owner_user_id;
  }
  if (wsInput) {
    wsInput.value = state.workspace_dir;
    if (state.resolved_workspace_placeholder) {
      wsInput.placeholder = state.resolved_workspace_placeholder;
    }
  }

  if (statusBadge) {
    const isConnected = state.connection_status.toLowerCase() === 'connected';
    statusBadge.textContent = isConnected ? 'Connected' : 'Disconnected';
    statusBadge.className = `channel-status-badge ${isConnected ? 'badge-connected' : 'badge-disconnected'}`;
  }

  if (policyWarning) {
    policyWarning.style.display = state.is_policy_covered ? 'none' : 'block';
  }

  if (statusMsg) {
    statusMsg.textContent = '';
  }

  allowlistTags = [...state.allowlist];
  renderAllowlistTags();

  setupEventListeners();
}

function renderAllowlistTags(): void {
  const container = document.getElementById('fs-allowlist-container');
  if (!container) return;

  container.innerHTML = '';
  allowlistTags.forEach((tag, idx) => {
    const tagEl = document.createElement('div');
    tagEl.className = 'channel-allowlist-tag';
    tagEl.innerHTML = `
      <span>${tag}</span>
      <button type="button" class="btn-remove-tag" data-index="${idx}" title="Remove">✕</button>
    `;

    tagEl.querySelector('.btn-remove-tag')?.addEventListener('click', (e) => {
      e.stopPropagation();
      allowlistTags.splice(idx, 1);
      renderAllowlistTags();
    });

    container.appendChild(tagEl);
  });
}

function setupEventListeners(): void {
  const wsInput = document.getElementById('input-fs-workspace-dir') as HTMLInputElement | null;
  const browseBtn = document.getElementById('btn-fs-pick-workspace');
  const addAllowlistBtn = document.getElementById('btn-fs-add-allowlist');
  const allowlistInput = document.getElementById('input-fs-allowlist') as HTMLInputElement | null;
  const testBtn = document.getElementById('btn-fs-test-connection');
  const saveBtn = document.getElementById('btn-fs-save');
  const policyWarning = document.getElementById('fs-policy-warning');
  const statusMsg = document.getElementById('fs-test-status-msg');

  // Policy coverage live checking
  wsInput?.addEventListener('input', () => {
    clearTimeout(policyDebounceTimer);
    policyDebounceTimer = window.setTimeout(async () => {
      const covered = await checkFeishuPolicyCoverageIpc(wsInput.value);
      if (policyWarning) {
        policyWarning.style.display = covered ? 'none' : 'block';
      }
    }, 250);
  });

  // Browse folder picker
  browseBtn?.addEventListener('click', async () => {
    const picked = await pickFeishuWorkspaceDialogIpc();
    if (picked && wsInput) {
      wsInput.value = picked;
      const covered = await checkFeishuPolicyCoverageIpc(picked);
      if (policyWarning) {
        policyWarning.style.display = covered ? 'none' : 'block';
      }
    }
  });

  // Add allowlist user
  const handleAddTag = () => {
    if (!allowlistInput) return;
    const val = allowlistInput.value.trim();
    if (val && !allowlistTags.includes(val)) {
      allowlistTags.push(val);
      allowlistInput.value = '';
      renderAllowlistTags();
    }
  };

  addAllowlistBtn?.addEventListener('click', handleAddTag);
  allowlistInput?.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleAddTag();
    }
  });

  // Test connection
  testBtn?.addEventListener('click', async () => {
    const domainSelect = document.getElementById('input-fs-domain') as HTMLSelectElement | null;
    const appIdInput = document.getElementById('input-fs-app-id') as HTMLInputElement | null;
    const appSecretInput = document.getElementById('input-fs-app-secret') as HTMLInputElement | null;

    const domain = domainSelect ? domainSelect.value : 'feishu';
    const appId = appIdInput ? appIdInput.value.trim() : '';
    const appSecret = appSecretInput ? appSecretInput.value.trim() : '';

    if (!appId || !appSecret) {
      if (statusMsg) {
        statusMsg.style.color = '#e74c3c';
        statusMsg.textContent = 'Please enter both App ID and App Secret.';
      }
      return;
    }

    if (statusMsg) {
      statusMsg.style.color = '#3498db';
      statusMsg.textContent = 'Testing Feishu credentials...';
    }

    try {
      const result = await testFeishuChannelConnectionIpc(appId, appSecret, domain);
      if (statusMsg) {
        statusMsg.style.color = '#2ecc71';
        statusMsg.textContent = result;
      }
    } catch (err: unknown) {
      if (statusMsg) {
        statusMsg.style.color = '#e74c3c';
        statusMsg.textContent = String(err);
      }
    }
  });

  // Save config
  saveBtn?.addEventListener('click', async () => {
    const domainSelect = document.getElementById('input-fs-domain') as HTMLSelectElement | null;
    const appIdInput = document.getElementById('input-fs-app-id') as HTMLInputElement | null;
    const appSecretInput = document.getElementById('input-fs-app-secret') as HTMLInputElement | null;
    const ownerInput = document.getElementById('input-fs-owner-user-id') as HTMLInputElement | null;
    const statusBadge = document.getElementById('fs-connection-badge');

    const domain = domainSelect ? domainSelect.value : 'feishu';
    const appId = appIdInput ? appIdInput.value.trim() : '';
    const appSecret = appSecretInput ? appSecretInput.value.trim() : '';
    const ownerUserId = ownerInput ? ownerInput.value.trim() : '';
    const workspaceDir = wsInput ? wsInput.value.trim() : '';

    if (!appId || !appSecret) {
      if (statusMsg) {
        statusMsg.style.color = '#e74c3c';
        statusMsg.textContent = 'App ID and App Secret are required.';
      }
      return;
    }

    const payload: SaveFeishuPayload = {
      app_id: appId,
      app_secret: appSecret,
      domain,
      owner_user_id: ownerUserId,
      allowlist: allowlistTags,
      workspace_dir: workspaceDir,
    };

    if (statusMsg) {
      statusMsg.style.color = '#3498db';
      statusMsg.textContent = 'Saving configuration & connecting...';
    }

    try {
      await saveFeishuChannelConfigIpc(payload);
      if (statusMsg) {
        statusMsg.style.color = '#2ecc71';
        statusMsg.textContent = '✓ Configuration saved & service initiated.';
      }
      if (statusBadge) {
        statusBadge.textContent = 'Connected';
        statusBadge.className = 'channel-status-badge badge-connected';
      }
    } catch (err: unknown) {
      if (statusMsg) {
        statusMsg.style.color = '#e74c3c';
        statusMsg.textContent = `Failed to save: ${err}`;
      }
    }
  });
}

