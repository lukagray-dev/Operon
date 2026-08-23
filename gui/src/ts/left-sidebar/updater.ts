// Left Sidebar Auto-Update & Relaunch Coordinator

import { invokeIpc, listenIpcEvent } from '../shared/ipc.js';

export interface UpdateInfo {
  version: string;
  title: string;
  body: string;
  html_url: string;
  published_at: string;
  download_url?: string | null;
}

let activeUpdateInfo: UpdateInfo | null = null;

export function getActiveUpdateInfo(): UpdateInfo | null {
  return activeUpdateInfo;
}

/**
 * Initializes the updater UI listeners and button event handler.
 */
export function initSidebarUpdater(): void {
  const container = document.getElementById('sidebar-update-container');
  const updateBtn = document.getElementById('btn-sidebar-update');
  const versionLabel = document.getElementById('update-version-label');

  if (!container || !updateBtn) return;

  // 1. Listen for updates ready for installation and application relaunch
  listenIpcEvent<UpdateInfo>('operon://update-ready', (info) => {
    if (!info || !info.version) return;
    activeUpdateInfo = info;

    if (versionLabel) {
      versionLabel.textContent = `v${info.version}`;
    }
    container.classList.remove('hidden');
    console.debug('[Updater] Update ready banner displayed for v' + info.version);
  });

  // 2. Listen for update available notifications
  listenIpcEvent<UpdateInfo>('operon://update-available', (info) => {
    if (!info || !info.version) return;
    activeUpdateInfo = info;

    if (versionLabel) {
      versionLabel.textContent = `v${info.version}`;
    }
    container.classList.remove('hidden');
    console.debug('[Updater] Update available for v' + info.version);
  });

  // 3. Handle click on Relaunch banner
  updateBtn.addEventListener('click', async () => {
    console.debug('[Updater] Relaunch requested by user...');
    updateBtn.style.opacity = '0.7';
    updateBtn.style.pointerEvents = 'none';

    try {
      await invokeIpc('relaunch_app');
    } catch (err) {
      console.error('[Updater] Failed to trigger relaunch:', err);
      updateBtn.style.opacity = '1';
      updateBtn.style.pointerEvents = 'auto';
    }
  });
}

/**
 * Manually checks for updates and returns update metadata if available.
 */
export async function manualCheckForUpdates(): Promise<UpdateInfo | null> {
  try {
    const res = await invokeIpc<UpdateInfo | null>('check_for_updates', { manual: true });
    return res || null;
  } catch (err) {
    console.error('[Updater] Manual check error:', err);
    throw err;
  }
}
