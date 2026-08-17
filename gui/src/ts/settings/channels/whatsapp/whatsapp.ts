// WhatsApp Channel Settings Controller
//
// 1:1 match with Slint settings/main-content/whatsapp.rs:
// - Manages Owner mobile number input.
// - Resolves custom workspace directory and evaluates live policy coverage.
// - Manages access permission allowlist of mobile numbers.
// - Handles QR pairing stream and mobile pairing code modal dialogs.

import {
  checkWhatsAppPolicyCoverageIpc,
  getWhatsAppStateIpc,
  pickWhatsAppWorkspaceDialogIpc,
  saveWhatsAppChannelConfigIpc,
  startWhatsAppCodePairingIpc,
  startWhatsAppQrPairingIpc,
} from './ipc.js';
import type { WhatsAppState } from './types.js';

let waState: WhatsAppState | null = null;
let waAllowlist: string[] = [];

/**
 * Initializes WhatsApp Channel panel.
 */
export async function initWhatsAppChannel(onSaved: () => Promise<void>): Promise<void> {
  setupWhatsAppForm(onSaved);
  setupWhatsAppModals();
  await refreshWhatsAppState();
}

/**
 * Refreshes WhatsApp state from the backend.
 */
export async function refreshWhatsAppState(): Promise<void> {
  try {
    waState = await getWhatsAppStateIpc();
    if (!waState) return;

    waAllowlist = [...waState.allowlist];
    populateWhatsAppFields();
  } catch (err) {
    console.error('[WhatsAppSettings] Failed to fetch state:', err);
  }
}

/**
 * Populates DOM elements with loaded WhatsApp state.
 */
function populateWhatsAppFields(): void {
  if (!waState) return;

  const statusBadge = document.getElementById('wa-status-badge');
  const ownerInput = document.getElementById('input-wa-owner-number') as HTMLInputElement | null;
  const wsInput = document.getElementById('input-wa-workspace-dir') as HTMLInputElement | null;

  if (statusBadge) {
    statusBadge.textContent = waState.connection_status;
    statusBadge.className = `channel-form-status-badge ${waState.connection_status === 'Connected' ? 'connected' : ''}`;
  }
  if (ownerInput) {
    ownerInput.value = waState.owner_number;
  }
  if (wsInput) {
    wsInput.value = waState.workspace_dir;
    wsInput.placeholder = waState.resolved_workspace_placeholder || '~/.operon/workspace';
  }

  updateWaPolicyBadge(waState.is_policy_covered);
  renderWhatsAppAllowlist();
}

/**
 * Updates WhatsApp workspace policy coverage badge.
 */
function updateWaPolicyBadge(isCovered: boolean): void {
  const badge = document.getElementById('wa-policy-badge');
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
 * Binds WhatsApp setup form inputs, buttons, and save action.
 */
function setupWhatsAppForm(onSaved: () => Promise<void>): void {
  const wsInput = document.getElementById('input-wa-workspace-dir') as HTMLInputElement | null;
  const browseBtn = document.getElementById('btn-wa-browse-workspace');
  const addAllowlistBtn = document.getElementById('btn-wa-add-allowlist');
  const allowlistInput = document.getElementById('input-wa-new-allowlist') as HTMLInputElement | null;
  const saveBtn = document.getElementById('btn-wa-save');

  // Live workspace policy check
  wsInput?.addEventListener('input', async () => {
    const isCovered = await checkWhatsAppPolicyCoverageIpc(wsInput.value.trim());
    updateWaPolicyBadge(isCovered);
  });

  // Browse folder dialog
  browseBtn?.addEventListener('click', async () => {
    const picked = await pickWhatsAppWorkspaceDialogIpc();
    if (picked && wsInput) {
      wsInput.value = picked;
      const isCovered = await checkWhatsAppPolicyCoverageIpc(picked);
      updateWaPolicyBadge(isCovered);
    }
  });

  // Add allowlist number
  addAllowlistBtn?.addEventListener('click', () => {
    if (!allowlistInput) return;
    const num = allowlistInput.value.trim();
    if (num && !waAllowlist.includes(num)) {
      waAllowlist.push(num);
      allowlistInput.value = '';
      renderWhatsAppAllowlist();
    }
  });

  // Save WhatsApp settings
  saveBtn?.addEventListener('click', async () => {
    const ownerInput = document.getElementById('input-wa-owner-number') as HTMLInputElement | null;
    const owner = ownerInput ? ownerInput.value.trim() : '';
    const ws = wsInput ? wsInput.value.trim() : '';

    try {
      await saveWhatsAppChannelConfigIpc({
        owner_number: owner,
        allowlist: waAllowlist,
        workspace_dir: ws,
      });
      await onSaved();
    } catch (err) {
      console.error('[WhatsAppSettings] Failed to save configuration:', err);
    }
  });
}

/**
 * Sets up QR and Pairing Code modal popups.
 */
function setupWhatsAppModals(): void {
  const scanQrBtn = document.getElementById('btn-wa-scan-qr');
  const pairCodeBtn = document.getElementById('btn-wa-pair-code');

  // QR Modal
  scanQrBtn?.addEventListener('click', async () => {
    const modal = document.getElementById('modal-wa-qr-popup');
    const qrContainer = document.getElementById('wa-qr-code-image-container');
    if (modal) modal.classList.remove('hidden');
    if (qrContainer) {
      qrContainer.innerHTML = '<div style="padding: 40px 20px; text-align: center; color: var(--text-muted); font-size: 13px;">Connecting to WhatsApp servers...</div>';
    }

    try {
      const qrSvg = await startWhatsAppQrPairingIpc();
      if (qrContainer) {
        qrContainer.innerHTML = qrSvg;
      }
    } catch (err) {
      if (qrContainer) {
        qrContainer.innerHTML = `<div style="padding: 40px 20px; text-align: center; color: #ef4444; font-size: 13px;">${err}</div>`;
      }
      console.warn('[WhatsAppSettings] QR generation error:', err);
    }
  });

  // Pairing Code Modal
  pairCodeBtn?.addEventListener('click', async () => {
    const ownerInput = document.getElementById('input-wa-owner-number') as HTMLInputElement | null;
    const phone = ownerInput ? ownerInput.value.trim() : '';

    if (!phone) {
      alert('Please enter your mobile phone number first.');
      return;
    }

    const modal = document.getElementById('modal-wa-pairing-code-popup');
    const codeEl = document.getElementById('wa-pairing-code-display');
    if (modal) modal.classList.remove('hidden');
    if (codeEl) {
      codeEl.textContent = 'Connecting...';
    }

    try {
      const code = await startWhatsAppCodePairingIpc(phone);
      if (codeEl) {
        codeEl.textContent = code;
      }
    } catch (err) {
      if (codeEl) {
        codeEl.textContent = `Error: ${err}`;
      }
      console.warn('[WhatsAppSettings] Pairing code error:', err);
    }
  });

  // Close actions
  document.getElementById('btn-wa-qr-close')?.addEventListener('click', () => {
    document.getElementById('modal-wa-qr-popup')?.classList.add('hidden');
  });
  document.getElementById('btn-wa-qr-close-top')?.addEventListener('click', () => {
    document.getElementById('modal-wa-qr-popup')?.classList.add('hidden');
  });

  document.getElementById('btn-wa-code-close')?.addEventListener('click', () => {
    document.getElementById('modal-wa-pairing-code-popup')?.classList.add('hidden');
  });
  document.getElementById('btn-wa-code-close-top')?.addEventListener('click', () => {
    document.getElementById('modal-wa-pairing-code-popup')?.classList.add('hidden');
  });
}

/**
 * Renders WhatsApp allowlist items.
 */
function renderWhatsAppAllowlist(): void {
  const container = document.getElementById('wa-allowlist-container');
  if (!container) return;

  container.innerHTML = '';
  waAllowlist.forEach((num, idx) => {
    const item = document.createElement('div');
    item.className = 'channel-allowlist-item';
    item.innerHTML = `
      <span class="channel-allowlist-text">${num}</span>
      <button class="channel-allowlist-del-btn" title="Remove">
        <span class="ui-icon icon-perm-delete"></span>
      </button>
    `;

    item.querySelector('.channel-allowlist-del-btn')?.addEventListener('click', () => {
      waAllowlist.splice(idx, 1);
      renderWhatsAppAllowlist();
    });

    container.appendChild(item);
  });
}
