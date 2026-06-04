'use strict';

/**
 * channels.js
 *
 * Channels (connectors) settings page for the Operon settings panel.
 *
 * Manages two views inside [data-channels-host]:
 *  1. List view  — all connectors with enabled/external toggles and a Setup button.
 *  2. Setup view — connector-specific credential fields, save, and (for WhatsApp)
 *                  native login QR code polling.
 *
 * Connector IDs supported: telegram, discord, whatsapp, email.
 */

import { showError, showSuccess } from '../../shared/toast.js';
import {
  activeCategory,
  renderInlineStatus,
  buildSettingRow,
  escapeHtml,
  normalizeErrorMessage,
} from '../settings-panel.js';
import {
  PLACEHOLDER_CONNECTORS,
  PLACEHOLDER_CONNECTOR_SETUPS,
  PLACEHOLDER_WHATSAPP_LOGIN,
} from './placeholders.js';

// ── Connector Icon Paths ──────────────────────────────────────────────────────

const CONNECTOR_ICON_PATHS = {
  telegram:  '../assets/icons/connector-telegram.svg',
  discord:   '../assets/icons/connector-discord.svg',
  whatsapp:  '../assets/icons/connector-whatsapp.svg',
  email:     '../assets/icons/connector-email.svg',
};

// ── WhatsApp polling interval ─────────────────────────────────────────────────
const WHATSAPP_POLL_INTERVAL_MS = 700;

// ── Transient Module State ────────────────────────────────────────────────────

const channelsState = {
  connectors: [],
  selectedConnectorId: '',
  activeView: 'list',
  setupByConnector: new Map(),
  draftByConnector: new Map(),
  whatsappLoginByConnector: new Map(),
  whatsappLoginPending: false,
  whatsappPollTimer: null,
  whatsappPollInFlight: false,
  loadingConnectors: false,
  loadingSetup: false,
  actionKey: '',
  saving: false,
  status: '',
};

// ── State Reset ───────────────────────────────────────────────────────────────

function resetChannelsSettingsState() {
  stopWhatsappLoginPolling();
  channelsState.connectors = [];
  channelsState.selectedConnectorId = '';
  channelsState.activeView = 'list';
  channelsState.setupByConnector.clear();
  channelsState.draftByConnector.clear();
  channelsState.whatsappLoginByConnector.clear();
  channelsState.whatsappLoginPending = false;
  channelsState.whatsappPollInFlight = false;
  channelsState.loadingConnectors = false;
  channelsState.loadingSetup = false;
  channelsState.actionKey = '';
  channelsState.saving = false;
  channelsState.status = '';
}

// ── Hydration Entry Point ─────────────────────────────────────────────────────

async function hydrateChannelsPage(modal) {
  if (!modal || activeCategory !== 'channels') return;

  if (channelsState.connectors.length === 0 && !channelsState.loadingConnectors) {
    channelsState.loadingConnectors = true;
    renderChannelsStage(modal);
    try {
      // PLACEHOLDER: Load connectors from static data
      await new Promise(resolve => setTimeout(resolve, 300)); // Simulate async
      const rows = PLACEHOLDER_CONNECTORS;
      channelsState.connectors = Array.isArray(rows) ? rows.map(normalizeConnectorRow) : [];
    } catch (error) {
      channelsState.connectors = [];
      showError(normalizeErrorMessage(error, 'Failed to load connectors.'));
    } finally {
      channelsState.loadingConnectors = false;
    }
  }

  if (
    channelsState.selectedConnectorId &&
    !channelsState.connectors.some(r => r.id === channelsState.selectedConnectorId)
  ) {
    channelsState.selectedConnectorId = '';
    channelsState.activeView = 'list';
  }

  renderChannelsStage(modal);
  if (channelsState.activeView === 'setup' && channelsState.selectedConnectorId) {
    await loadConnectorSetup(modal, channelsState.selectedConnectorId);
  }
}

// ── Stage Renderer ────────────────────────────────────────────────────────────

function renderChannelsStage(modal) {
  const host = modal?.querySelector('[data-channels-host]');
  if (!host) return;

  // Stop WhatsApp polling if we're leaving the WhatsApp setup view
  const keepPolling =
    channelsState.activeView === 'setup' &&
    channelsState.selectedConnectorId === 'whatsapp' &&
    channelsState.whatsappLoginPending;
  if (!keepPolling) stopWhatsappLoginPolling();

  if (channelsState.loadingConnectors) {
    host.innerHTML = `<div class="settings-channels__view">${renderInlineStatus('Loading connectors...', true)}</div>`;
    return;
  }

  if (channelsState.connectors.length === 0) {
    host.innerHTML = `<div class="settings-channels__view">${renderInlineStatus('No connectors available.')}</div>`;
    return;
  }

  if (channelsState.activeView === 'setup' && channelsState.selectedConnectorId) {
    host.innerHTML = renderConnectorSetupView();
    bindConnectorSetupEvents(modal);
    return;
  }

  host.innerHTML = renderConnectorsListView();
  bindConnectorListEvents(modal);
}

// ── List View ─────────────────────────────────────────────────────────────────

function renderConnectorsListView() {
  const rows = channelsState.connectors.map(connector => {
    const iconPath = CONNECTOR_ICON_PATHS[connector.id] || '';
    const iconHtml = iconPath
      ? `<img src="${iconPath}" alt="${escapeHtml(connector.label)}" width="18" height="18" draggable="false">`
      : '';
    const isBusy = channelsState.actionKey === `toggle-enabled:${connector.id}` ||
                   channelsState.actionKey === `toggle-external:${connector.id}`;
    const controlsDisabled = Boolean(channelsState.actionKey) && !isBusy;

    return `
      <div class="settings-channels__connector-row">
        <div class="settings-channels__connector-info">
          <span class="settings-channels__connector-icon">${iconHtml}</span>
          <span class="settings-channels__connector-title">${escapeHtml(connector.label)}</span>
          <span class="settings-badge ${connector.enabled ? 'settings-badge--success' : ''}">
            ${connector.enabled ? 'Enabled' : 'Disabled'}
          </span>
          <span class="settings-badge ${connector.externalAccessEnabled ? 'settings-badge--info' : ''}">
            ${connector.externalAccessEnabled ? 'External' : 'Owner only'}
          </span>
        </div>
        <div class="settings-channels__connector-actions">
          <button class="btn btn--secondary btn--sm"
                  type="button"
                  data-connector-open="${escapeHtml(connector.id)}"
                  ${controlsDisabled ? 'disabled' : ''}>
            Setup
          </button>
          <button class="btn btn--ghost btn--sm"
                  type="button"
                  data-connector-toggle-enabled="${escapeHtml(connector.id)}"
                  ${controlsDisabled ? 'disabled' : ''}>
            ${channelsState.actionKey === `toggle-enabled:${connector.id}`
              ? '<span class="model-selector__spinner" aria-hidden="true"></span>'
              : ''}
            ${connector.enabled ? 'Disable' : 'Enable'}
          </button>
          <button class="btn btn--ghost btn--sm"
                  type="button"
                  data-connector-toggle-external="${escapeHtml(connector.id)}"
                  ${controlsDisabled ? 'disabled' : ''}>
            ${connector.externalAccessEnabled ? 'External on' : 'External off'}
          </button>
        </div>
      </div>
    `;
  }).join('');

  return `
    <div class="settings-channels__view settings-channels__view--list">
      ${channelsState.loadingConnectors ? renderInlineStatus('Loading connectors...', true) : (rows || renderInlineStatus('No connectors available.'))}
      ${channelsState.status ? `<div class="settings-channels__status">${escapeHtml(channelsState.status)}</div>` : ''}
    </div>
  `;
}

function bindConnectorListEvents(modal) {
  const host = modal?.querySelector('[data-channels-host]');
  if (!host) return;

  host.querySelectorAll('[data-connector-open]').forEach(btn => {
    btn.addEventListener('click', () => {
      const id = String(btn.getAttribute('data-connector-open') || '').trim();
      if (id) void openConnectorSetup(modal, id);
    });
  });

  host.querySelectorAll('[data-connector-toggle-enabled]').forEach(btn => {
    btn.addEventListener('click', () => {
      const id  = String(btn.getAttribute('data-connector-toggle-enabled') || '').trim();
      const row = channelsState.connectors.find(c => c.id === id);
      if (id && row) void setConnectorEnabled(modal, id, !row.enabled, 'list');
    });
  });

  host.querySelectorAll('[data-connector-toggle-external]').forEach(btn => {
    btn.addEventListener('click', () => {
      const id  = String(btn.getAttribute('data-connector-toggle-external') || '').trim();
      const row = channelsState.connectors.find(c => c.id === id);
      if (id && row) void setConnectorExternalAccess(modal, id, !row.externalAccessEnabled, 'list');
    });
  });
}

// ── Setup View ────────────────────────────────────────────────────────────────

function renderConnectorSetupView() {
  const connectorId = channelsState.selectedConnectorId;
  if (!connectorId) {
    return `<div class="settings-channels__view settings-channels__view--setup">${renderInlineStatus('Select a connector to configure.')}</div>`;
  }
  const connector = channelsState.connectors.find(c => c.id === connectorId);
  if (!connector) {
    return `<div class="settings-channels__view settings-channels__view--setup">${renderInlineStatus('Connector metadata is unavailable.')}</div>`;
  }

  const topbar = `
    <div class="settings-models__topbar">
      <button class="btn btn--ghost btn--sm" type="button" data-channels-back>
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
             fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="15 18 9 12 15 6"/>
        </svg>
        Back
      </button>
    </div>
  `;

  if (channelsState.loadingSetup) {
    return `<div class="settings-channels__view settings-channels__view--setup">${topbar}${renderInlineStatus('Loading connector setup...', true)}</div>`;
  }

  const setup = channelsState.setupByConnector.get(connectorId);
  if (!setup) {
    return `<div class="settings-channels__view settings-channels__view--setup">${topbar}${renderInlineStatus('Connector setup not loaded.')}</div>`;
  }

  const draft = getConnectorDraft(connectorId, setup);
  const togglesLocked = Boolean(channelsState.actionKey) || channelsState.saving;

  return `
    <div class="settings-channels__view settings-channels__view--setup">
      ${topbar}

      ${buildSettingRow('Enabled', 'Toggle connector enabled state', `
        <label class="settings-row__toggle">
          <input type="checkbox" data-connector-enabled-checkbox ${draft.enabled ? 'checked' : ''} ${togglesLocked ? 'disabled' : ''}>
          <span class="settings-row__toggle-slider"></span>
        </label>
      `)}

      ${buildSettingRow('External senders', 'Allow external senders to use this connector', `
        <label class="settings-row__toggle">
          <input type="checkbox" data-connector-external-checkbox ${draft.externalAccessEnabled ? 'checked' : ''} ${togglesLocked ? 'disabled' : ''}>
          <span class="settings-row__toggle-slider"></span>
        </label>
      `)}

      ${renderConnectorSetupFields(setup, draft)}

      <div class="settings-row">
        <div class="settings-row__info"></div>
        <div class="settings-row__control" style="gap: 8px;">
          <button class="btn btn--ghost btn--sm" type="button" data-connector-refresh
                  ${channelsState.loadingSetup || channelsState.saving ? 'disabled' : ''}>
            Reload
          </button>
          <button class="btn btn--primary btn--sm" type="button" data-connector-save
                  ${channelsState.saving ? 'disabled' : ''}>
            ${channelsState.saving ? '<span class="model-selector__spinner" aria-hidden="true"></span> Saving...' : 'Save'}
          </button>
        </div>
      </div>

      ${channelsState.status ? `<div class="settings-channels__status">${escapeHtml(channelsState.status)}</div>` : ''}
    </div>
  `;
}

function renderConnectorSetupFields(setup, draft) {
  const allowFromInput = escapeHtml(draft.allowFromInput || '');
  const externalHint   = 'Comma, semicolon, or whitespace separated.';

  if (setup.connectorId === 'telegram') {
    return `
      ${buildSettingRow('Bot token', 'Telegram bot token for API access', `
        <input class="settings-models__input" type="password" data-connector-telegram-token value="${escapeHtml(draft.telegramToken)}" placeholder="Paste Telegram bot token">
      `)}
      ${buildSettingRow('Allowlist', externalHint, `
        <input class="settings-models__input" type="text" data-connector-allow-from value="${allowFromInput}" placeholder="123456789, @username">
      `)}
    `;
  }

  if (setup.connectorId === 'discord') {
    return `
      ${buildSettingRow('Bot token', 'Discord bot token for API access', `
        <input class="settings-models__input" type="password" data-connector-discord-token value="${escapeHtml(draft.discordToken)}" placeholder="Paste Discord bot token">
      `)}
      ${buildSettingRow('Allowlist', externalHint, `
        <input class="settings-models__input" type="text" data-connector-allow-from value="${allowFromInput}" placeholder="123456789012345678, @username">
      `)}
    `;
  }

  if (setup.connectorId === 'whatsapp') {
    const snapshot = channelsState.whatsappLoginByConnector.get('whatsapp') || { sessionStorePath: '', qrText: '', pairCode: '', connected: false };
    const qrDisplayText = resolveWhatsappQrDisplayText(snapshot);
    return `
      ${buildSettingRow('Mode', 'Use native WhatsApp library or bridge server', `
        <label class="settings-row__toggle">
          <input type="checkbox" data-connector-whatsapp-native ${draft.whatsappUseNative ? 'checked' : ''}>
          <span class="settings-row__toggle-slider"></span>
        </label>
      `)}
      ${buildSettingRow('Bridge URL', 'WebSocket URL for WhatsApp bridge (bridge mode only)', `
        <input class="settings-models__input" type="text" data-connector-whatsapp-bridge-url value="${escapeHtml(draft.whatsappBridgeUrl)}" placeholder="ws://localhost:3001" ${draft.whatsappUseNative ? 'disabled' : ''}>
      `)}
      ${buildSettingRow('Session store path', 'Directory to store WhatsApp session data', `
        <input class="settings-models__input" type="text" data-connector-whatsapp-session-store value="${escapeHtml(draft.whatsappSessionStorePath)}" placeholder="<workspace>/whatsapp">
      `)}
      ${buildSettingRow('Allowlist', externalHint, `
        <input class="settings-models__input" type="text" data-connector-allow-from value="${allowFromInput}" placeholder="+1234567890, +1987654321">
      `)}
      <section class="settings-channels__whatsapp-login">
        <div class="settings-channels__whatsapp-login-header">
          <span class="settings-channels__whatsapp-login-title">Native login</span>
          <div class="settings-channels__whatsapp-login-actions">
            <button class="btn btn--secondary btn--sm" type="button" data-connector-whatsapp-prepare-login ${!draft.whatsappUseNative || channelsState.actionKey ? 'disabled' : ''}>
              ${channelsState.actionKey === 'whatsapp-login' ? '<span class="model-selector__spinner" aria-hidden="true"></span> Preparing...' : 'Prepare login'}
            </button>
            <button class="btn btn--ghost btn--sm" type="button" data-connector-whatsapp-refresh-login ${!draft.whatsappUseNative || channelsState.actionKey ? 'disabled' : ''}>
              Refresh
            </button>
          </div>
        </div>
        <div class="settings-channels__whatsapp-login-meta">
          <span class="settings-badge ${snapshot.connected ? 'settings-badge--success' : ''}">${snapshot.connected ? 'Connected' : 'Not connected'}</span>
          ${snapshot.pairCode ? `<span class="settings-badge">Pair ${escapeHtml(snapshot.pairCode)}</span>` : ''}
        </div>
        <div class="settings-channels__whatsapp-login-path">${escapeHtml(snapshot.sessionStorePath || draft.whatsappSessionStorePath || '(session path not set)')}</div>
        <pre class="settings-channels__whatsapp-qr">${escapeHtml(qrDisplayText)}</pre>
      </section>
    `;
  }

  if (setup.connectorId === 'email') {
    return `
      ${buildSettingRow('Email address', 'Email address for sending and receiving', `
        <input class="settings-models__input" type="email" data-connector-email-address value="${escapeHtml(draft.emailAddress)}" placeholder="you@example.com">
      `)}
      ${buildSettingRow('App password', 'Application-specific password', `
        <input class="settings-models__input" type="password" data-connector-email-password value="${escapeHtml(draft.emailPassword)}" placeholder="Paste app password">
      `)}
      <div class="settings-channels__grid">
        ${buildSettingRow('IMAP host', 'IMAP server hostname', `<input class="settings-models__input" type="text" data-connector-email-imap-host value="${escapeHtml(draft.emailImapHost)}" placeholder="imap.gmail.com">`)}
        ${buildSettingRow('IMAP port', 'IMAP server port', `<input class="settings-models__input" type="number" min="1" data-connector-email-imap-port value="${escapeHtml(draft.emailImapPort)}" placeholder="993">`)}
      </div>
      <div class="settings-channels__grid">
        ${buildSettingRow('SMTP host', 'SMTP server hostname', `<input class="settings-models__input" type="text" data-connector-email-smtp-host value="${escapeHtml(draft.emailSmtpHost)}" placeholder="smtp.gmail.com">`)}
        ${buildSettingRow('SMTP port', 'SMTP server port', `<input class="settings-models__input" type="number" min="1" data-connector-email-smtp-port value="${escapeHtml(draft.emailSmtpPort)}" placeholder="587">`)}
      </div>
      <div class="settings-channels__grid">
        ${buildSettingRow('Mailbox', 'Mailbox to monitor for incoming emails', `<input class="settings-models__input" type="text" data-connector-email-mailbox value="${escapeHtml(draft.emailMailbox)}" placeholder="INBOX">`)}
        ${buildSettingRow('Poll interval', 'Seconds between email checks', `<input class="settings-models__input" type="number" min="1" data-connector-email-poll-interval value="${escapeHtml(draft.emailPollIntervalSecs)}" placeholder="60">`)}
      </div>
      ${buildSettingRow('Allowlist', externalHint, `
        <input class="settings-models__input" type="text" data-connector-allow-from value="${allowFromInput}" placeholder="sender@example.com, @domain.com">
      `)}
      ${buildSettingRow('Display name', 'Sender name for outgoing emails', `
        <input class="settings-models__input" type="text" data-connector-email-display-name value="${escapeHtml(draft.emailDisplayName)}" placeholder="Operon Agent">
      `)}
    `;
  }

  return renderInlineStatus('Connector setup is not implemented.');
}

function resolveWhatsappQrDisplayText(snapshot) {
  if (snapshot.connected) return '(Connected — no QR needed)';
  if (snapshot.qrText)   return snapshot.qrText;
  if (snapshot.pairCode) return `Pair code: ${snapshot.pairCode}`;
  return '(No QR code yet — click Prepare login)';
}

function bindConnectorSetupEvents(modal) {
  const host = modal?.querySelector('[data-channels-host]');
  if (!host) return;

  host.querySelector('[data-channels-back]')?.addEventListener('click', () => goBackToConnectorList(modal));
  host.querySelector('[data-connector-refresh]')?.addEventListener('click', () => {
    void loadConnectorSetup(modal, channelsState.selectedConnectorId, true);
  });
  host.querySelector('[data-connector-save]')?.addEventListener('click', () => {
    void saveSelectedConnectorSetup(modal);
  });

  host.querySelector('[data-connector-enabled-checkbox]')?.addEventListener('change', event => {
    const target = event.currentTarget;
    if (target instanceof HTMLInputElement) {
      void setConnectorEnabled(modal, channelsState.selectedConnectorId, target.checked, 'setup');
    }
  });

  host.querySelector('[data-connector-external-checkbox]')?.addEventListener('change', event => {
    const target = event.currentTarget;
    if (target instanceof HTMLInputElement) {
      void setConnectorExternalAccess(modal, channelsState.selectedConnectorId, target.checked, 'setup');
    }
  });

  host.querySelector('[data-connector-whatsapp-native]')?.addEventListener('change', () => {
    const draft = syncConnectorDraftFromInputs(modal);
    if (!draft?.whatsappUseNative) {
      channelsState.whatsappLoginPending = false;
      stopWhatsappLoginPolling();
      channelsState.status = '';
    }
    renderChannelsStage(modal);
  });

  host.querySelector('[data-connector-whatsapp-prepare-login]')?.addEventListener('click', () => {
    void prepareWhatsappLogin(modal);
  });

  host.querySelector('[data-connector-whatsapp-refresh-login]')?.addEventListener('click', () => {
    void refreshWhatsappLoginSnapshot(modal);
  });

  // Sync draft on any input change
  const draftSelectors = [
    '[data-connector-telegram-token]', '[data-connector-discord-token]',
    '[data-connector-allow-from]', '[data-connector-whatsapp-bridge-url]',
    '[data-connector-whatsapp-session-store]', '[data-connector-email-address]',
    '[data-connector-email-password]', '[data-connector-email-imap-host]',
    '[data-connector-email-imap-port]', '[data-connector-email-smtp-host]',
    '[data-connector-email-smtp-port]', '[data-connector-email-mailbox]',
    '[data-connector-email-poll-interval]', '[data-connector-email-display-name]',
  ];
  draftSelectors.forEach(sel => {
    host.querySelector(sel)?.addEventListener('input', () => syncConnectorDraftFromInputs(modal));
  });
}

// ── Navigation ────────────────────────────────────────────────────────────────

async function openConnectorSetup(modal, connectorId) {
  if (connectorId !== 'whatsapp') {
    channelsState.whatsappLoginPending = false;
    stopWhatsappLoginPolling();
  }
  channelsState.selectedConnectorId = connectorId;
  channelsState.activeView = 'setup';
  channelsState.status = '';
  renderChannelsStage(modal);
  await loadConnectorSetup(modal, connectorId);
}

function goBackToConnectorList(modal) {
  channelsState.whatsappLoginPending = false;
  stopWhatsappLoginPolling();
  syncConnectorDraftFromInputs(modal);
  channelsState.activeView = 'list';
  channelsState.status = '';
  renderChannelsStage(modal);
}

// ── Data Loading / Saving ─────────────────────────────────────────────────────

async function loadConnectorSetup(modal, connectorId, force = false) {
  if (!modal || !connectorId) { renderChannelsStage(modal); return; }
  if (!force && channelsState.setupByConnector.has(connectorId)) { renderChannelsStage(modal); return; }

  channelsState.loadingSetup = true;
  channelsState.status = 'Loading connector setup...';
  renderChannelsStage(modal);

  try {
    // PLACEHOLDER: Load connector setup with delay
    await new Promise(resolve => setTimeout(resolve, 300));
    const payload = PLACEHOLDER_CONNECTOR_SETUPS.get(connectorId) || { connectorId };
    const normalized = normalizeConnectorSetup(payload);
    channelsState.setupByConnector.set(connectorId, normalized);
    syncConnectorSummary(connectorId, normalized);
    if (force || !channelsState.draftByConnector.has(connectorId)) {
      channelsState.draftByConnector.set(connectorId, connectorDraftFromSetup(normalized));
    }
    if (connectorId === 'whatsapp') {
      if (!normalized.whatsappUseNative) {
        channelsState.whatsappLoginPending = false;
        stopWhatsappLoginPolling();
      }
      try {
        // PLACEHOLDER: WhatsApp login snapshot
        await new Promise(resolve => setTimeout(resolve, 200));
        const snapshot = PLACEHOLDER_WHATSAPP_LOGIN;
        const norm     = applyWhatsappLoginSnapshot(snapshot);
        if (channelsState.whatsappLoginPending && !norm.connected && activeCategory === 'channels') {
          startWhatsappLoginPolling(modal);
        }
      } catch { /* non-critical */ }
    } else {
      channelsState.whatsappLoginPending = false;
      stopWhatsappLoginPolling();
    }
    channelsState.status = '';
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to load connector setup.'));
  } finally {
    channelsState.loadingSetup = false;
    renderChannelsStage(modal);
  }
}

function connectorDraftFromSetup(setup) {
  return {
    enabled: Boolean(setup.enabled),
    externalAccessEnabled: Boolean(setup.externalAccessEnabled),
    allowFromInput: (setup.allowFrom || []).join(', '),
    telegramToken: String(setup.telegramToken || ''),
    discordToken: String(setup.discordToken || ''),
    whatsappBridgeUrl: String(setup.whatsappBridgeUrl || ''),
    whatsappUseNative: Boolean(setup.whatsappUseNative),
    whatsappSessionStorePath: String(setup.whatsappSessionStorePath || ''),
    emailAddress: String(setup.emailAddress || ''),
    emailPassword: String(setup.emailPassword || ''),
    emailImapHost: String(setup.emailImapHost || ''),
    emailImapPort: String(setup.emailImapPort ?? ''),
    emailSmtpHost: String(setup.emailSmtpHost || ''),
    emailSmtpPort: String(setup.emailSmtpPort ?? ''),
    emailMailbox: String(setup.emailMailbox || ''),
    emailPollIntervalSecs: String(setup.emailPollIntervalSecs ?? ''),
    emailDisplayName: String(setup.emailDisplayName || ''),
  };
}

function getConnectorDraft(connectorId, setup) {
  const existing = channelsState.draftByConnector.get(connectorId);
  if (existing) return existing;
  const seeded = connectorDraftFromSetup(setup);
  channelsState.draftByConnector.set(connectorId, seeded);
  return seeded;
}

function syncConnectorDraftFromInputs(modal) {
  const connectorId = channelsState.selectedConnectorId;
  if (!connectorId) return null;

  const host = modal?.querySelector('[data-channels-host]');
  const setup = channelsState.setupByConnector.get(connectorId);
  const draft = channelsState.draftByConnector.get(connectorId) || connectorDraftFromSetup(setup || {});

  if (!host) { channelsState.draftByConnector.set(connectorId, draft); return { ...draft }; }

  const setVal = (selector, key) => {
    const el = host.querySelector(selector);
    if (el instanceof HTMLInputElement) draft[key] = el.type === 'checkbox' ? el.checked : el.value;
  };

  setVal('[data-connector-allow-from]', 'allowFromInput');
  setVal('[data-connector-telegram-token]', 'telegramToken');
  setVal('[data-connector-discord-token]', 'discordToken');
  setVal('[data-connector-whatsapp-bridge-url]', 'whatsappBridgeUrl');
  setVal('[data-connector-whatsapp-native]', 'whatsappUseNative');
  setVal('[data-connector-whatsapp-session-store]', 'whatsappSessionStorePath');
  setVal('[data-connector-email-address]', 'emailAddress');
  setVal('[data-connector-email-password]', 'emailPassword');
  setVal('[data-connector-email-imap-host]', 'emailImapHost');
  setVal('[data-connector-email-imap-port]', 'emailImapPort');
  setVal('[data-connector-email-smtp-host]', 'emailSmtpHost');
  setVal('[data-connector-email-smtp-port]', 'emailSmtpPort');
  setVal('[data-connector-email-mailbox]', 'emailMailbox');
  setVal('[data-connector-email-poll-interval]', 'emailPollIntervalSecs');
  setVal('[data-connector-email-display-name]', 'emailDisplayName');
  setVal('[data-connector-enabled-checkbox]', 'enabled');
  setVal('[data-connector-external-checkbox]', 'externalAccessEnabled');

  channelsState.draftByConnector.set(connectorId, draft);
  return { ...draft };
}

async function saveSelectedConnectorSetup(modal) {
  const connectorId = channelsState.selectedConnectorId;
  if (!connectorId || channelsState.saving) return;

  const form      = readConnectorSetupForm(modal);
  const allowFrom = parseAllowFromInput(form.allowFromInput);
  channelsState.saving = true;
  channelsState.status = `Saving ${connectorId} connector setup...`;
  renderChannelsStage(modal);

  try {
    // PLACEHOLDER: Simulate saving connector setup with delay
    await new Promise(resolve => setTimeout(resolve, 400));
    
    // Create simulated payload based on form data
    const payload = {
      connectorId,
      enabled: form.enabled,
      externalAccessEnabled: form.externalAccessEnabled,
      allowFrom,
    };

    // Add connector-specific fields
    if (connectorId === 'telegram') {
      payload.telegramToken = String(form.telegramToken || '').trim();
    } else if (connectorId === 'discord') {
      payload.discordToken = String(form.discordToken || '').trim();
    } else if (connectorId === 'whatsapp') {
      payload.whatsappBridgeUrl = String(form.whatsappBridgeUrl || '').trim();
      payload.whatsappUseNative = Boolean(form.whatsappUseNative);
      payload.whatsappSessionStorePath = String(form.whatsappSessionStorePath || '').trim();
    } else if (connectorId === 'email') {
      payload.emailAddress = String(form.emailAddress || '').trim();
      payload.emailPassword = String(form.emailPassword || '').trim();
      payload.emailImapHost = String(form.emailImapHost || '').trim();
      payload.emailImapPort = Number(form.emailImapPort);
      payload.emailSmtpHost = String(form.emailSmtpHost || '').trim();
      payload.emailSmtpPort = Number(form.emailSmtpPort);
      payload.emailMailbox = String(form.emailMailbox || '').trim();
      payload.emailPollIntervalSecs = Number(form.emailPollIntervalSecs);
      payload.emailDisplayName = String(form.emailDisplayName || '').trim();
    } else {
      throw new Error(`Unknown connector '${connectorId}'`);
    }

    const normalized = normalizeConnectorSetup(payload);
    channelsState.setupByConnector.set(connectorId, normalized);
    channelsState.draftByConnector.set(connectorId, connectorDraftFromSetup(normalized));
    syncConnectorSummary(connectorId, normalized);
    channelsState.status = 'Connector setup saved.';
    showSuccess('Connector setup saved.');
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to save connector setup.'));
  } finally {
    channelsState.saving = false;
    renderChannelsStage(modal);
  }
}

function readConnectorSetupForm(modal) {
  const draft = syncConnectorDraftFromInputs(modal);
  if (draft) return draft;
  const fallback = channelsState.draftByConnector.get(channelsState.selectedConnectorId);
  return fallback ? { ...fallback } : connectorDraftFromSetup({});
}

async function setConnectorEnabled(modal, connectorId, enabled, view) {
  if (channelsState.actionKey) return;
  channelsState.actionKey = `toggle-enabled:${connectorId}`;
  renderChannelsStage(modal);
  try {
    // PLACEHOLDER: Simulate toggling connector enabled state
    await new Promise(resolve => setTimeout(resolve, 300));
    
    const setup = channelsState.setupByConnector.get(connectorId) || { connectorId };
    const payload = { ...setup, enabled };
    const norm = normalizeConnectorSetup(payload);
    channelsState.setupByConnector.set(connectorId, norm);
    channelsState.draftByConnector.set(connectorId, connectorDraftFromSetup(norm));
    syncConnectorSummary(connectorId, norm);
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to toggle connector.'));
  } finally {
    channelsState.actionKey = '';
    renderChannelsStage(modal);
  }
}

async function setConnectorExternalAccess(modal, connectorId, enabled, view) {
  if (channelsState.actionKey) return;
  channelsState.actionKey = `toggle-external:${connectorId}`;
  renderChannelsStage(modal);
  try {
    // PLACEHOLDER: Simulate toggling external access
    await new Promise(resolve => setTimeout(resolve, 300));
    
    const setup = channelsState.setupByConnector.get(connectorId) || { connectorId };
    const payload = { ...setup, externalAccessEnabled: enabled };
    const norm = normalizeConnectorSetup(payload);
    channelsState.setupByConnector.set(connectorId, norm);
    channelsState.draftByConnector.set(connectorId, connectorDraftFromSetup(norm));
    syncConnectorSummary(connectorId, norm);
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to toggle external access.'));
  } finally {
    channelsState.actionKey = '';
    renderChannelsStage(modal);
  }
}

function syncConnectorSummary(connectorId, setup) {
  channelsState.connectors = channelsState.connectors.map(c =>
    c.id !== connectorId ? c : {
      ...c,
      enabled: Boolean(setup.enabled),
      externalAccessEnabled: Boolean(setup.externalAccessEnabled),
    }
  );
}

// ── WhatsApp Login Polling ────────────────────────────────────────────────────

async function prepareWhatsappLogin(modal) {
  if (channelsState.actionKey) return;
  channelsState.actionKey = 'whatsapp-login';
  renderChannelsStage(modal);
  try {
    // PLACEHOLDER: Simulate preparing WhatsApp login
    await new Promise(resolve => setTimeout(resolve, 400));
    const snapshot = {
      ...PLACEHOLDER_WHATSAPP_LOGIN,
      qrText: '(Mock QR code - scan with WhatsApp app)',
      pairCode: '1234-5678',
    };
    applyWhatsappLoginSnapshot(snapshot);
    channelsState.whatsappLoginPending = true;
    startWhatsappLoginPolling(modal);
  } catch (error) {
    showError(normalizeErrorMessage(error, 'Failed to prepare WhatsApp login.'));
  } finally {
    channelsState.actionKey = '';
    renderChannelsStage(modal);
  }
}

async function refreshWhatsappLoginSnapshot(modal) {
  try {
    // PLACEHOLDER: Refresh WhatsApp login snapshot
    await new Promise(resolve => setTimeout(resolve, 200));
    const snapshot = PLACEHOLDER_WHATSAPP_LOGIN;
    applyWhatsappLoginSnapshot(snapshot);
    renderChannelsStage(modal);
  } catch { /* non-critical refresh */ }
}

function applyWhatsappLoginSnapshot(snapshot) {
  const normalized = {
    sessionStorePath: String(snapshot?.sessionStorePath || ''),
    qrText:           String(snapshot?.qrText || ''),
    pairCode:         String(snapshot?.pairCode || ''),
    connected:        Boolean(snapshot?.connected),
  };
  channelsState.whatsappLoginByConnector.set('whatsapp', normalized);
  if (normalized.connected) {
    channelsState.whatsappLoginPending = false;
    stopWhatsappLoginPolling();
  }
  return normalized;
}

function startWhatsappLoginPolling(modal) {
  if (channelsState.whatsappPollTimer) return;
  channelsState.whatsappPollTimer = setInterval(async () => {
    if (channelsState.whatsappPollInFlight || activeCategory !== 'channels') return;
    channelsState.whatsappPollInFlight = true;
    try {
      // PLACEHOLDER: Poll WhatsApp login snapshot
      await new Promise(resolve => setTimeout(resolve, 200));
      const snapshot = PLACEHOLDER_WHATSAPP_LOGIN;
      applyWhatsappLoginSnapshot(snapshot);
      renderChannelsStage(modal);
    } catch { /* ignore polling errors */ } finally {
      channelsState.whatsappPollInFlight = false;
    }
  }, WHATSAPP_POLL_INTERVAL_MS);
}

function stopWhatsappLoginPolling() {
  if (channelsState.whatsappPollTimer) {
    clearInterval(channelsState.whatsappPollTimer);
    channelsState.whatsappPollTimer = null;
  }
  channelsState.whatsappPollInFlight = false;
}

// ── Normalizers / Helpers ─────────────────────────────────────────────────────

function normalizeConnectorRow(row) {
  return {
    id:                    String(row?.id || '').trim(),
    label:                 String(row?.label || '').trim() || 'Connector',
    enabled:               Boolean(row?.enabled),
    externalAccessEnabled: Boolean(row?.externalAccessEnabled),
  };
}

function normalizeConnectorSetup(row) {
  return {
    connectorId:           String(row?.connectorId || '').trim(),
    label:                 String(row?.label || '').trim() || 'Connector',
    docsUrl:               String(row?.docsUrl || '').trim(),
    enabled:               Boolean(row?.enabled),
    externalAccessEnabled: Boolean(row?.externalAccessEnabled),
    allowFrom:             Array.isArray(row?.allowFrom) ? row.allowFrom : [],
    telegramToken:         String(row?.telegramToken || ''),
    discordToken:          String(row?.discordToken || ''),
    whatsappBridgeUrl:     String(row?.whatsappBridgeUrl || ''),
    whatsappUseNative:     Boolean(row?.whatsappUseNative),
    whatsappSessionStorePath: String(row?.whatsappSessionStorePath || ''),
    emailAddress:          String(row?.emailAddress || ''),
    emailPassword:         String(row?.emailPassword || ''),
    emailImapHost:         String(row?.emailImapHost || ''),
    emailImapPort:         row?.emailImapPort ?? '',
    emailSmtpHost:         String(row?.emailSmtpHost || ''),
    emailSmtpPort:         row?.emailSmtpPort ?? '',
    emailMailbox:          String(row?.emailMailbox || ''),
    emailPollIntervalSecs: row?.emailPollIntervalSecs ?? '',
    emailDisplayName:      String(row?.emailDisplayName || ''),
  };
}

function parseAllowFromInput(input) {
  return String(input || '')
    .split(/[\s,;]+/)
    .map(s => s.trim())
    .filter(Boolean);
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { resetChannelsSettingsState, hydrateChannelsPage };
