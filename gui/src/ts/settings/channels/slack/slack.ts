// Slack Channel Settings Controller
//
// Manages Slack Bot Token, App Token (Socket Mode), Owner User ID, Allowlist, and Workspace Directory.

import {
  checkSlackPolicyCoverageIpc,
  getSlackStateIpc,
  pickSlackWorkspaceDialogIpc,
  saveSlackChannelConfigIpc,
  testSlackChannelConnectionIpc,
} from './ipc.js';
import type { SlackStateDto } from './types.js';

let slState: SlackStateDto | null = null;
let slAllowlist: string[] = [];

/**
 * Initializes Slack Channel panel.
 */
export async function initSlackChannel(onSaved: () => Promise<void>): Promise<void> {
  setupSlackForm(onSaved);
  await refreshSlackState();
}

/**
 * Refreshes Slack state from the backend.
 */
export async function refreshSlackState(): Promise<void> {
  try {
    slState = await getSlackStateIpc();
    if (!slState) return;

    slAllowlist = [...slState.allowlist];
    populateSlackFields();
  } catch (err) {
    console.error('[SlackSettings] Failed to fetch state:', err);
  }
}

/**
 * Populates DOM elements with loaded Slack state.
 */
function populateSlackFields(): void {
  if (!slState) return;

  const statusBadge = document.getElementById('sl-status-badge');
  const botTokenInput = document.getElementById('input-sl-bot-token') as HTMLInputElement | null;
  const appTokenInput = document.getElementById('input-sl-app-token') as HTMLInputElement | null;
  const ownerInput = document.getElementById('input-sl-owner-user-id') as HTMLInputElement | null;
  const wsInput = document.getElementById('input-sl-workspace-dir') as HTMLInputElement | null;

  if (statusBadge) {
    statusBadge.textContent = slState.connection_status;
    statusBadge.className = `channel-form-status-badge ${slState.connection_status === 'Connected' ? 'connected' : ''}`;
  }
  if (botTokenInput) {
    botTokenInput.value = slState.bot_token;
  }
  if (appTokenInput) {
    appTokenInput.value = slState.app_token;
  }
  if (ownerInput) {
    ownerInput.value = slState.owner_user_id;
  }
  if (wsInput) {
    wsInput.value = slState.workspace_dir;
    wsInput.placeholder = slState.resolved_workspace_placeholder || '~/.operon/workspace';
  }

  updateSlPolicyBadge(slState.is_policy_covered);
  renderSlackAllowlist();
}

/**
 * Updates Slack workspace policy coverage badge.
 */
function updateSlPolicyBadge(isCovered: boolean): void {
  const badge = document.getElementById('sl-policy-badge');
  if (!badge) return;

  if (isCovered) {
    badge.textContent = '✓ Policy Covered';
    badge.className = 'channel-policy-badge covered';
  } else {
    badge.textContent = '⚠ Uncovered by Policy (Tool calls will Deny)';
    badge.className = 'channel-policy-badge uncovered';
  }
}

/**
 * Binds Slack setup form inputs, test connection button, and save action.
 */
function setupSlackForm(onSaved: () => Promise<void>): void {
  const wsInput = document.getElementById('input-sl-workspace-dir') as HTMLInputElement | null;
  const browseBtn = document.getElementById('btn-sl-browse-workspace');
  const addAllowlistBtn = document.getElementById('btn-sl-add-allowlist');
  const allowlistInput = document.getElementById('input-sl-new-allowlist') as HTMLInputElement | null;
  const testBtn = document.getElementById('btn-sl-test-connection');
  const saveBtn = document.getElementById('btn-sl-save');
  const statusMsg = document.getElementById('sl-test-status-msg');

  // Live workspace policy check
  wsInput?.addEventListener('input', async () => {
    const isCovered = await checkSlackPolicyCoverageIpc(wsInput.value.trim());
    updateSlPolicyBadge(isCovered);
  });

  // Browse folder dialog
  browseBtn?.addEventListener('click', async () => {
    const picked = await pickSlackWorkspaceDialogIpc();
    if (picked && wsInput) {
      wsInput.value = picked;
      const isCovered = await checkSlackPolicyCoverageIpc(picked);
      updateSlPolicyBadge(isCovered);
    }
  });

  // Add allowlist ID
  addAllowlistBtn?.addEventListener('click', () => {
    if (!allowlistInput) return;
    const id = allowlistInput.value.trim();
    if (id && !slAllowlist.includes(id)) {
      slAllowlist.push(id);
      allowlistInput.value = '';
      renderSlackAllowlist();
    }
  });

  // Test connection
  testBtn?.addEventListener('click', async () => {
    const botTokenInput = document.getElementById('input-sl-bot-token') as HTMLInputElement | null;
    const token = botTokenInput ? botTokenInput.value.trim() : '';

    if (!token) {
      if (statusMsg) statusMsg.textContent = 'Please enter a bot token first.';
      return;
    }

    if (statusMsg) statusMsg.textContent = 'Testing bot token via auth.test...';
    try {
      const res = await testSlackChannelConnectionIpc(token);
      if (statusMsg) statusMsg.textContent = `✓ ${res}`;
    } catch (err) {
      if (statusMsg) statusMsg.textContent = `⚠ ${err}`;
    }
  });

  // Save Slack settings
  saveBtn?.addEventListener('click', async () => {
    const botTokenInput = document.getElementById('input-sl-bot-token') as HTMLInputElement | null;
    const appTokenInput = document.getElementById('input-sl-app-token') as HTMLInputElement | null;
    const ownerInput = document.getElementById('input-sl-owner-user-id') as HTMLInputElement | null;
    const botToken = botTokenInput ? botTokenInput.value.trim() : '';
    const appToken = appTokenInput ? appTokenInput.value.trim() : '';
    const owner = ownerInput ? ownerInput.value.trim() : '';
    const ws = wsInput ? wsInput.value.trim() : '';

    try {
      await saveSlackChannelConfigIpc({
        bot_token: botToken,
        app_token: appToken,
        owner_user_id: owner,
        allowlist: slAllowlist,
        workspace_dir: ws,
      });
      await onSaved();
    } catch (err) {
      console.error('[SlackSettings] Failed to save configuration:', err);
    }
  });
}

/**
 * Renders Slack allowlist items.
 */
function renderSlackAllowlist(): void {
  const container = document.getElementById('sl-allowlist-container');
  if (!container) return;

  container.innerHTML = '';
  slAllowlist.forEach((id, idx) => {
    const item = document.createElement('div');
    item.className = 'channel-allowlist-item';
    item.innerHTML = `
      <span class="channel-allowlist-text">${id}</span>
      <button class="channel-allowlist-del-btn" title="Remove">
        <span class="ui-icon icon-perm-delete"></span>
      </button>
    `;

    item.querySelector('.channel-allowlist-del-btn')?.addEventListener('click', () => {
      slAllowlist.splice(idx, 1);
      renderSlackAllowlist();
    });

    container.appendChild(item);
  });
}

