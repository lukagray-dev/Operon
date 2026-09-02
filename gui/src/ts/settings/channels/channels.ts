// Channels Root Coordinator
//
// Manages high-level view navigation between Channels List (0), WhatsApp Setup (1), Telegram Setup (2), Discord Setup (3), Slack Setup (4), and Feishu Setup (5).

import { getChannelsListIpc } from './ipc.js';
import { initTelegramChannel, refreshTelegramState } from './telegram/telegram.js';
import { initDiscordChannel, refreshDiscordState } from './discord/discord.js';
import { initSlackChannel, refreshSlackState } from './slack/slack.js';
import { initFeishuView } from './feishu/feishu.js';
import type { ChannelCard } from './types.js';
import { initWhatsAppChannel, refreshWhatsAppState } from './whatsapp/whatsapp.js';

let activeView = 0; // 0 = List, 1 = WhatsApp, 2 = Telegram, 3 = Discord, 4 = Slack, 5 = Feishu
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
  await initDiscordChannel(handleSaved);
  await initSlackChannel(handleSaved);
  await initFeishuView();
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
    await refreshDiscordState();
    await refreshSlackState();
    await initFeishuView();
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

    let iconClass = 'icon-channel-whatsapp';
    if (ch.id === 'telegram') {
      iconClass = 'icon-channel-telegram';
    } else if (ch.id === 'discord') {
      iconClass = 'icon-channel-discord';
    } else if (ch.id === 'slack') {
      iconClass = 'icon-channel-slack';
    } else if (ch.id === 'feishu') {
      iconClass = 'icon-channel-feishu';
    }

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

    card.addEventListener('click', async () => {
      if (ch.id === 'whatsapp') {
        activeView = 1;
        await refreshWhatsAppState();
      } else if (ch.id === 'telegram') {
        activeView = 2;
        await refreshTelegramState();
      } else if (ch.id === 'discord') {
        activeView = 3;
        await refreshDiscordState();
      } else if (ch.id === 'slack') {
        activeView = 4;
        await refreshSlackState();
      } else if (ch.id === 'feishu') {
        activeView = 5;
        await initFeishuView();
      }
      updateChannelsViewSwitch();
    });

    container.appendChild(card);
  });
}

/**
 * Sets up back button navigation for channel views.
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

  document.getElementById('btn-dc-back')?.addEventListener('click', () => {
    activeView = 0;
    updateChannelsViewSwitch();
  });

  document.getElementById('btn-sl-back')?.addEventListener('click', () => {
    activeView = 0;
    updateChannelsViewSwitch();
  });

  document.getElementById('btn-back-to-channels-feishu')?.addEventListener('click', () => {
    activeView = 0;
    updateChannelsViewSwitch();
  });
}

/**
 * Updates UI view container visibility between List, WhatsApp, Telegram, Discord, Slack, and Feishu.
 */
function updateChannelsViewSwitch(): void {
  const listView = document.getElementById('channels-view-list');
  const waView = document.getElementById('channels-view-whatsapp');
  const tgView = document.getElementById('channels-view-telegram');
  const dcView = document.getElementById('channels-view-discord');
  const slView = document.getElementById('channels-view-slack');
  const fsView = document.getElementById('channels-view-feishu');
  const headerSubtitle = document.getElementById('channels-header-subtitle');

  if (activeView === 0) {
    listView?.classList.remove('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.add('hidden');
    dcView?.classList.add('hidden');
    slView?.classList.add('hidden');
    if (fsView) fsView.style.display = 'none';
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Set up messaging channel integrations such as WhatsApp, Telegram, Discord, Slack, or Feishu / Lark.';
    }
  } else if (activeView === 1) {
    listView?.classList.add('hidden');
    waView?.classList.remove('hidden');
    tgView?.classList.add('hidden');
    dcView?.classList.add('hidden');
    slView?.classList.add('hidden');
    if (fsView) fsView.style.display = 'none';
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure WhatsApp mobile pairing, Owner number, and access permission allowlist.';
    }
  } else if (activeView === 2) {
    listView?.classList.add('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.remove('hidden');
    dcView?.classList.add('hidden');
    slView?.classList.add('hidden');
    if (fsView) fsView.style.display = 'none';
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure Telegram Bot Token, Owner Chat ID, and access permission allowlist.';
    }
  } else if (activeView === 3) {
    listView?.classList.add('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.add('hidden');
    dcView?.classList.remove('hidden');
    slView?.classList.add('hidden');
    if (fsView) fsView.style.display = 'none';
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure Discord Bot Token, Owner User ID, and access permission allowlist.';
    }
  } else if (activeView === 4) {
    listView?.classList.add('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.add('hidden');
    dcView?.classList.add('hidden');
    slView?.classList.remove('hidden');
    if (fsView) fsView.style.display = 'none';
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure Slack Bot Token, App-Level Token (Socket Mode), Owner User ID, and access permission allowlist.';
    }
  } else if (activeView === 5) {
    listView?.classList.add('hidden');
    waView?.classList.add('hidden');
    tgView?.classList.add('hidden');
    dcView?.classList.add('hidden');
    slView?.classList.add('hidden');
    if (fsView) fsView.style.display = 'flex';
    if (headerSubtitle) {
      headerSubtitle.textContent = 'Configure Feishu / Lark App ID, App Secret, Domain, Owner User ID, and access permission allowlist.';
    }
  }
}
