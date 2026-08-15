// Telegram Channel Settings Controller
//
// 1:1 match with Slint settings/main-content/telegram.rs:
// - Manages Bot token and Owner Chat ID inputs.
// - Resolves custom workspace directory and evaluates live policy coverage.
// - Manages access permission allowlist of numeric chat IDs.
// - Executes real-time Bot Token validation test (via TelegramClient / getMe).

import {
  checkTelegramPolicyCoverageIpc,
  getTelegramStateIpc,
  pickTelegramWorkspaceDialogIpc,
  saveTelegramChannelConfigIpc,
  testTelegramChannelConnectionIpc,
} from './ipc.js';
import type { TelegramState } from './types.js';

let tgState: TelegramState | null = null;
let tgAllowlist: string[] = [];

/**
 * Initializes Telegram Channel panel.
 */
export async function initTelegramChannel(onSaved: () => Promise<void>): Promise<void> {
  setupTelegramForm(onSaved);
  await refreshTelegramState();
}

/**
 * Refreshes Telegram state from the backend.
 */
export async function refreshTelegramState(): Promise<void> {
  try {
    tgState = await getTelegramStateIpc();
    if (!tgState) return;

    tgAllowlist = [...tgState.allowlist];
    populateTelegramFields();
  } catch (err) {
    console.error('[TelegramSettings] Failed to fetch state:', err);
  }
}

/**
 * Populates DOM elements with loaded Telegram state.
 */
function populateTelegramFields(): void {
  if (!tgState) return;

  const statusBadge = document.getElementById('tg-status-badge');
  const tokenInput = document.getElementById('input-tg-bot-token') as HTMLInputElement | null;
  const ownerInput = document.getElementById('input-tg-owner-chat-id') as HTMLInputElement | null;
  const wsInput = document.getElementById('input-tg-workspace-dir') as HTMLInputElement | null;

  if (statusBadge) {
    statusBadge.textContent = tgState.connection_status;
    statusBadge.className = `channel-form-status-badge ${tgState.connection_status === 'Connected' ? 'connected' : ''}`;
  }
  if (tokenInput) {
    tokenInput.value = tgState.bot_token;
  }
  if (ownerInput) {
    ownerInput.value = tgState.owner_chat_id;
  }
  if (wsInput) {
    wsInput.value = tgState.workspace_dir;
    wsInput.placeholder = tgState.resolved_workspace_placeholder || '~/.operon/workspace';
  }

  updateTgPolicyBadge(tgState.is_policy_covered);
  renderTelegramAllowlist();
}

/**
 * Updates Telegram workspace policy coverage badge.
 */
function updateTgPolicyBadge(isCovered: boolean): void {
  const badge = document.getElementById('tg-policy-badge');
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
 * Binds Telegram setup form inputs, test connection button, and save action.
 */
function setupTelegramForm(onSaved: () => Promise<void>): void {
  const wsInput = document.getElementById('input-tg-workspace-dir') as HTMLInputElement | null;
  const browseBtn = document.getElementById('btn-tg-browse-workspace');
  const addAllowlistBtn = document.getElementById('btn-tg-add-allowlist');
  const allowlistInput = document.getElementById('input-tg-new-allowlist') as HTMLInputElement | null;
  const testBtn = document.getElementById('btn-tg-test-connection');
  const saveBtn = document.getElementById('btn-tg-save');
  const statusMsg = document.getElementById('tg-test-status-msg');

  // Live workspace policy check
  wsInput?.addEventListener('input', async () => {
    const isCovered = await checkTelegramPolicyCoverageIpc(wsInput.value.trim());
    updateTgPolicyBadge(isCovered);
  });

  // Browse folder dialog
  browseBtn?.addEventListener('click', async () => {
    const picked = await pickTelegramWorkspaceDialogIpc();
    if (picked && wsInput) {
      wsInput.value = picked;
      const isCovered = await checkTelegramPolicyCoverageIpc(picked);
      updateTgPolicyBadge(isCovered);
    }
  });

  // Add allowlist ID
  addAllowlistBtn?.addEventListener('click', () => {
    if (!allowlistInput) return;
    const id = allowlistInput.value.trim();
    if (id && !tgAllowlist.includes(id)) {
      tgAllowlist.push(id);
      allowlistInput.value = '';
      renderTelegramAllowlist();
    }
  });

  // Test connection
  testBtn?.addEventListener('click', async () => {
    const tokenInput = document.getElementById('input-tg-bot-token') as HTMLInputElement | null;
    const token = tokenInput ? tokenInput.value.trim() : '';

    if (!token) {
      if (statusMsg) statusMsg.textContent = 'Please enter a bot token first.';
      return;
    }

    if (statusMsg) statusMsg.textContent = 'Testing bot token via getMe...';
    try {
      const res = await testTelegramChannelConnectionIpc(token);
      if (statusMsg) statusMsg.textContent = `✓ ${res}`;
    } catch (err) {
      if (statusMsg) statusMsg.textContent = `⚠ ${err}`;
    }
  });

  // Save Telegram settings
  saveBtn?.addEventListener('click', async () => {
    const tokenInput = document.getElementById('input-tg-bot-token') as HTMLInputElement | null;
    const ownerInput = document.getElementById('input-tg-owner-chat-id') as HTMLInputElement | null;
    const token = tokenInput ? tokenInput.value.trim() : '';
    const owner = ownerInput ? ownerInput.value.trim() : '';
    const ws = wsInput ? wsInput.value.trim() : '';

    try {
      await saveTelegramChannelConfigIpc({
        bot_token: token,
        owner_chat_id: owner,
        allowlist: tgAllowlist,
        workspace_dir: ws,
      });
      await onSaved();
    } catch (err) {
      console.error('[TelegramSettings] Failed to save configuration:', err);
    }
  });
}

/**
 * Renders Telegram allowlist items.
 */
function renderTelegramAllowlist(): void {
  const container = document.getElementById('tg-allowlist-container');
  if (!container) return;

  container.innerHTML = '';
  tgAllowlist.forEach((id, idx) => {
    const item = document.createElement('div');
    item.className = 'channel-allowlist-item';
    item.innerHTML = `
      <span class="channel-allowlist-text">${id}</span>
      <button class="channel-allowlist-del-btn" title="Remove">
        <span class="ui-icon icon-perm-delete"></span>
      </button>
    `;

    item.querySelector('.channel-allowlist-del-btn')?.addEventListener('click', () => {
      tgAllowlist.splice(idx, 1);
      renderTelegramAllowlist();
    });

    container.appendChild(item);
  });
}
