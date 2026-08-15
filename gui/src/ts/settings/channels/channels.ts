// Channels Root Coordinator
//
// Manages high-level view navigation between Channels List (0), WhatsApp Setup (1), and Telegram Setup (2).

import { getChannelsListIpc } from './ipc.js';
import { initTelegramChannel, refreshTelegramState } from './telegram/telegram.js';
import type { ChannelCard } from './types.js';
import { initWhatsAppChannel, refreshWhatsAppState } from './whatsapp/whatsapp.js';

let activeView = 0; // 0 = List, 1 = WhatsApp, 2 = Telegram
let channelsList: ChannelCard[] = [];

/**
 * Initializes Channels settings module.
 */
export async function initChannelsSettings(): Promise<void> {
  setupNavigationButtons();

  const handleSaved = async () => {
    await refreshChannelsData();
    activeView = 0;
    updateChannelsViewSwitch();
  };

  await initWhatsAppChannel(handleSaved);
  await initTelegramChannel(handleSaved);
  await refreshChannelsData();
}

/**
 * Refreshes channels list cards and submodules.
 */
export async function refreshChannelsData(): Promise<void> {
  try {
    channelsList = await getChannelsListIpc();
    renderChannelsList();
    await refreshWhatsAppState();
    await refreshTelegramState();
  } catch (err) {
    console.error('[ChannelsSettings] Failed to fetch channels list:', err);
  }
}

/**
 * Renders channels summary list cards.
 */
function renderChannelsList(): void {
  const container = document.getElementById('channels-cards-container');
  if (!container) return;

  container.innerHTML = '';

  channelsList.forEach((ch) => {
    const card = document.createElement('div');
    card.className = 'channel-card';
    card.dataset.id = ch.id;

    const iconClass = ch.id === 'whatsapp' ? 'icon-channel-whatsapp' : 'icon-channel-telegram';

    card.innerHTML = `
      <div class="channel-icon-wrapper">
        <span class="channel-icon ${iconClass}"></span>
      </div>
      <div class="channel-info">
        <div class="channel-label">${ch.label}</div>
        <div class="channel-desc">${ch.description}</div>
      </div>
      <div class="channel-action">
        ${
          ch.is_active
            ? '<span class="channel-connected-badge">Connected</span>'
            : '<span class="channel-disconnected-badge">Disconnected</span>'
        }
        <span class="ui-icon icon-chevron-right channel-chevron"></span>
      </div>
    `;

    card.addEventListener('click', () => {
      if (ch.id === 'whatsapp') {
        activeView = 1;
      } else if (ch.id === 'telegram') {
        activeView = 2;
      }
      updateChannelsViewSwitch();
    });

    container.appendChild(card);
  });
}

/**
 * Sets up back button navigation for both channel views.
 */
function setupNavigationButtons(): void {
  document.getElementById('btn-wa-back')?.addEventListener('click', () => {
    activeView = 0;
    updateChannelsViewSwitch();
  });

  document.getElementById('btn-tg-back')?.addEventListener('click', () => {
    activeView = 0;
    updateChannelsViewSwitch();
  });
}

/**
 * Updates UI view container visibility between List, WhatsApp, and Telegram.
 */
function updateChannelsViewSwitch(): void {
  const listView = document.getElementById('channels-view-list');
  const waView = document.getElementById('channels-view-whatsapp');
  const tgView = document.getElementById('channels-view-telegram');
  const headerSubtitle = document.getElementById('channels-header-subtitle');

  if (activeView === 0) {
    listView?.classList.remove('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.add('hidden');
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Set up messaging channel integrations such as WhatsApp, Telegram, or Discord.';
    }
  } else if (activeView === 1) {
    listView?.classList.add('hidden');
    waView?.classList.remove('hidden');
    tgView?.classList.add('hidden');
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure WhatsApp mobile pairing, Owner number, and access permission allowlist.';
    }
  } else if (activeView === 2) {
    listView?.classList.add('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.remove('hidden');
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure Telegram Bot Token, Owner Chat ID, and access permission allowlist.';
    }
  }
}
