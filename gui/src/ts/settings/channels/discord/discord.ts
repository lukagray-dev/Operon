// Discord Channel Settings Controller
//
// Manages Discord Bot Token, Owner User ID, Allowlist, and Workspace Directory configuration.

import {
  checkDiscordPolicyCoverageIpc,
  getDiscordStateIpc,
  pickDiscordWorkspaceDialogIpc,
  saveDiscordChannelConfigIpc,
  testDiscordChannelConnectionIpc,
} from './ipc.js';
import type { DiscordState } from './types.js';

let dcState: DiscordState | null = null;
let dcAllowlist: string[] = [];

/**
 * Initializes Discord Channel panel.
 */
export async function initDiscordChannel(onSaved: () => Promise<void>): Promise<void> {
  setupDiscordForm(onSaved);
  await refreshDiscordState();
}

/**
 * Refreshes Discord state from the backend.
 */
export async function refreshDiscordState(): Promise<void> {
  try {
    dcState = await getDiscordStateIpc();
    if (!dcState) return;

    dcAllowlist = [...dcState.allowlist];
    populateDiscordFields();
  } catch (err) {
    console.error('[DiscordSettings] Failed to fetch state:', err);
  }
}

/**
 * Populates DOM elements with loaded Discord state.
 */
function populateDiscordFields(): void {
  if (!dcState) return;

  const statusBadge = document.getElementById('dc-status-badge');
  const tokenInput = document.getElementById('input-dc-bot-token') as HTMLInputElement | null;
  const ownerInput = document.getElementById('input-dc-owner-user-id') as HTMLInputElement | null;
  const guildInput = document.getElementById('input-dc-guild-id') as HTMLInputElement | null;
  const wsInput = document.getElementById('input-dc-workspace-dir') as HTMLInputElement | null;

  if (statusBadge) {
    statusBadge.textContent = dcState.connection_status;
    statusBadge.className = `channel-form-status-badge ${dcState.connection_status === 'Connected' ? 'connected' : ''}`;
  }
  if (tokenInput) {
    tokenInput.value = dcState.bot_token;
  }
  if (ownerInput) {
    ownerInput.value = dcState.owner_user_id;
  }
  if (guildInput) {
    guildInput.value = dcState.guild_id;
  }
  if (wsInput) {
    wsInput.value = dcState.workspace_dir;
    wsInput.placeholder = dcState.resolved_workspace_placeholder || '~/.operon/workspace';
  }

  updateDcPolicyBadge(dcState.is_policy_covered);
  renderDiscordAllowlist();
}

/**
 * Updates Discord workspace policy coverage badge.
 */
function updateDcPolicyBadge(isCovered: boolean): void {
  const badge = document.getElementById('dc-policy-badge');
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
 * Binds Discord setup form inputs, test connection button, and save action.
 */
function setupDiscordForm(onSaved: () => Promise<void>): void {
  const wsInput = document.getElementById('input-dc-workspace-dir') as HTMLInputElement | null;
  const browseBtn = document.getElementById('btn-dc-browse-workspace');
  const addAllowlistBtn = document.getElementById('btn-dc-add-allowlist');
  const allowlistInput = document.getElementById('input-dc-new-allowlist') as HTMLInputElement | null;
  const testBtn = document.getElementById('btn-dc-test-connection');
  const saveBtn = document.getElementById('btn-dc-save');
  const statusMsg = document.getElementById('dc-test-status-msg');

  // Live workspace policy check
  wsInput?.addEventListener('input', async () => {
    const isCovered = await checkDiscordPolicyCoverageIpc(wsInput.value.trim());
    updateDcPolicyBadge(isCovered);
  });

  // Browse folder dialog
  browseBtn?.addEventListener('click', async () => {
    const picked = await pickDiscordWorkspaceDialogIpc();
    if (picked && wsInput) {
      wsInput.value = picked;
      const isCovered = await checkDiscordPolicyCoverageIpc(picked);
      updateDcPolicyBadge(isCovered);
    }
  });

  // Add allowlist ID
  addAllowlistBtn?.addEventListener('click', () => {
    if (!allowlistInput) return;
    const id = allowlistInput.value.trim();
    if (id && !dcAllowlist.includes(id)) {
      dcAllowlist.push(id);
      allowlistInput.value = '';
      renderDiscordAllowlist();
    }
  });

  // Test connection
  testBtn?.addEventListener('click', async () => {
    const tokenInput = document.getElementById('input-dc-bot-token') as HTMLInputElement | null;
    const token = tokenInput ? tokenInput.value.trim() : '';

    if (!token) {
      if (statusMsg) statusMsg.textContent = 'Please enter a bot token first.';
      return;
    }

    if (statusMsg) statusMsg.textContent = 'Testing bot token via /users/@me...';
    try {
      const res = await testDiscordChannelConnectionIpc(token);
      if (statusMsg) statusMsg.textContent = `✓ ${res}`;
    } catch (err) {
      if (statusMsg) statusMsg.textContent = `⚠ ${err}`;
    }
  });

  // Save Discord settings
  saveBtn?.addEventListener('click', async () => {
    const tokenInput = document.getElementById('input-dc-bot-token') as HTMLInputElement | null;
    const ownerInput = document.getElementById('input-dc-owner-user-id') as HTMLInputElement | null;
    const guildInput = document.getElementById('input-dc-guild-id') as HTMLInputElement | null;
    const token = tokenInput ? tokenInput.value.trim() : '';
    const owner = ownerInput ? ownerInput.value.trim() : '';
    const guild = guildInput ? guildInput.value.trim() : '';
    const ws = wsInput ? wsInput.value.trim() : '';

    try {
      await saveDiscordChannelConfigIpc({
        bot_token: token,
        owner_user_id: owner,
        allowlist: dcAllowlist,
        guild_id: guild,
        workspace_dir: ws,
      });
      await onSaved();
    } catch (err) {
      console.error('[DiscordSettings] Failed to save configuration:', err);
    }
  });
}

/**
 * Renders Discord allowlist items.
 */
function renderDiscordAllowlist(): void {
  const container = document.getElementById('dc-allowlist-container');
  if (!container) return;

  container.innerHTML = '';
  dcAllowlist.forEach((id, idx) => {
    const item = document.createElement('div');
    item.className = 'channel-allowlist-item';
    item.innerHTML = `
      <span class="channel-allowlist-text">${id}</span>
      <button class="channel-allowlist-del-btn" title="Remove">
        <span class="ui-icon icon-perm-delete"></span>
      </button>
    `;

    item.querySelector('.channel-allowlist-del-btn')?.addEventListener('click', () => {
      dcAllowlist.splice(idx, 1);
      renderDiscordAllowlist();
    });

    container.appendChild(item);
  });
}

