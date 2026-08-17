// Settings Window Root Coordinator

import { invokeIpc } from '../shared/ipc.js';
import { initAboutSettings } from './about/about.js';
import { initAppearanceSettings } from './appearance/appearance.js';
import { initChannelsSettings } from './channels/channels.js';
import { initGeneralSettings } from './general/general.js';
import { closeSettingsWindowIpc } from './ipc.js';
import { initMemorySettings } from './memory/memory.js';
import { initModelsSettings } from './models/models.js';
import { initPermissionsSettings } from './permissions/permissions.js';
import { initSettingsSidebar } from './sidebar/sidebar.js';

let isMaximized = false;

window.addEventListener('DOMContentLoaded', async () => {
  initSettingsTitlebar();
  initSettingsSidebar();
  await initGeneralSettings();
  await initAppearanceSettings();
  await initModelsSettings();
  await initPermissionsSettings();
  await initChannelsSettings();
  await initMemorySettings();
  await initAboutSettings();
  console.debug('[Operon Settings] Window initialized with General, Appearance, Models, Permissions, Channels, Memory & About panels.');
});

/**
 * Initializes custom titlebar controls for the standalone Settings window.
 */
function initSettingsTitlebar(): void {
  const minBtn = document.getElementById('btn-minimize');
  const maxBtn = document.getElementById('btn-maximize');
  const closeBtn = document.getElementById('btn-close');
  const maxIcon = document.getElementById('icon-max-restore');
  const titlebar = document.getElementById('settings-titlebar');
  const dragSpacer = document.querySelector('.titlebar-drag-spacer');

  // Minimize
  minBtn?.addEventListener('click', async (e) => {
    e.stopPropagation();
    await invokeIpc('minimize_window');
  });

  // Maximize / Restore
  maxBtn?.addEventListener('click', async (e) => {
    e.stopPropagation();
    const isMax = await invokeIpc<boolean>('toggle_maximize_window');
    if (isMax !== null) {
      updateMaximizeIcon(isMax, maxIcon);
    }
  });

  // Close Settings Window
  closeBtn?.addEventListener('click', async (e) => {
    e.stopPropagation();
    await closeSettingsWindowIpc();
  });

  // Dragging and double-click maximization support:
  // Differentiates single click (drag) from double click (toggle maximize) using e.detail
  titlebar?.addEventListener('mousedown', async (e) => {
    const target = e.target as HTMLElement | null;
    if (target?.closest('.action-btn') || target?.closest('button')) {
      return;
    }
    if (e.button === 0) {
      if (
        target === titlebar ||
        target === dragSpacer ||
        target?.classList.contains('titlebar-left') ||
        target?.classList.contains('titlebar') ||
        target?.closest('.titlebar-left')
      ) {
        if (e.detail % 2 === 0) {
          // Double-click: Toggle maximize / restore
          const isMax = await invokeIpc<boolean>('toggle_maximize_window');
          if (isMax !== null) {
            updateMaximizeIcon(isMax, maxIcon);
          }
        } else {
          // Single-click: Start dragging
          try {
            await invokeIpc('start_dragging');
          } catch {
            // Window drag error handled gracefully
          }
        }
      }
    }
  });

  // Synchronize maximize icon state with current window status
  const syncMaximized = async () => {
    const isMax = await invokeIpc<boolean>('is_window_maximized');
    if (isMax !== null) {
      updateMaximizeIcon(isMax, maxIcon);
    }
  };

  // Check initial maximized state
  syncMaximized();

  // Listen to window resize events to keep the maximize/restore icon synchronized (for native double-clicks & OS snap)
  window.addEventListener('resize', syncMaximized);
}

function updateMaximizeIcon(max: boolean, iconEl: HTMLElement | null): void {
  isMaximized = max;
  if (iconEl) {
    if (isMaximized) {
      iconEl.classList.remove('icon-maximize');
      iconEl.classList.add('icon-unmaxmize');
    } else {
      iconEl.classList.remove('icon-unmaxmize');
      iconEl.classList.add('icon-maximize');
    }
  }
}
