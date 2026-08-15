// General Settings Controller & DOM Coordinator
//
// 1:1 implementation matching Slint general.slint:
// - Section 1: System & Window Behavior (Autostart, Minimize to Tray, Start Minimized, Close Action Choice)
// - Section 2: Session & Chat Defaults (Auto-Approve, Auto-Scroll, Permission Notifications, Finish Notifications, Auto-Collapse Cards)
// - Section 3: Updates & Diagnostics (Auto Update Checks, Anonymous Diagnostics)

import { getGeneralSettingsIpc, saveGeneralSettingsIpc } from './ipc.js';
import type { GeneralSettings } from './types.js';

let currentSettings: GeneralSettings = {
  autostart_enabled: false,
  minimize_to_tray_enabled: false,
  start_minimized: false,
  close_button_action: 0,
  global_auto_approve_default: false,
  auto_scroll_stream: true,
  notify_on_permission_request: true,
  notify_on_response_complete: false,
  auto_collapse_reasoning_tools: false,
  auto_update_checks: true,
  telemetry_enabled: false,
};

/**
 * Initializes the General Settings panel and binds UI interactions.
 */
export async function initGeneralSettings(): Promise<void> {
  try {
    currentSettings = await getGeneralSettingsIpc();
  } catch (err) {
    console.warn('[GeneralSettings] Failed to load settings:', err);
  }

  setupToggleSwitches();
  setupSegmentedChoices();
  syncGeneralUI();
}

/**
 * Binds all toggle switch controls in the General settings panel.
 */
function setupToggleSwitches(): void {
  // 1. Autostart
  bindSwitch('toggle-gen-autostart', currentSettings.autostart_enabled, async (val) => {
    currentSettings.autostart_enabled = val;
    await persist();
  });

  // 2. Minimize to Tray (Core prerequisite for Start Minimized & Close to Tray)
  bindSwitch('toggle-gen-tray', currentSettings.minimize_to_tray_enabled, async (val) => {
    currentSettings.minimize_to_tray_enabled = val;
    if (!val) {
      currentSettings.start_minimized = false;
      currentSettings.close_button_action = 0; // Force Exit App
    }
    syncGeneralUI();
    await persist();
  });

  // 3. Start Minimized
  bindSwitch('toggle-gen-start-min', currentSettings.start_minimized, async (val) => {
    currentSettings.start_minimized = val;
    await persist();
  });

  // 4. Auto-Approve Default
  bindSwitch('toggle-gen-auto-approve', currentSettings.global_auto_approve_default, async (val) => {
    currentSettings.global_auto_approve_default = val;
    await persist();
  });

  // 5. Auto-Scroll Stream
  bindSwitch('toggle-gen-auto-scroll', currentSettings.auto_scroll_stream, async (val) => {
    currentSettings.auto_scroll_stream = val;
    await persist();
  });

  // 6. Notify on Permission
  bindSwitch('toggle-gen-notify-perm', currentSettings.notify_on_permission_request, async (val) => {
    currentSettings.notify_on_permission_request = val;
    await persist();
  });

  // 7. Notify on Response Complete
  bindSwitch('toggle-gen-notify-complete', currentSettings.notify_on_response_complete, async (val) => {
    currentSettings.notify_on_response_complete = val;
    await persist();
  });

  // 8. Auto-Collapse Reasoning & Tool Cards
  bindSwitch('toggle-gen-auto-collapse', currentSettings.auto_collapse_reasoning_tools, async (val) => {
    currentSettings.auto_collapse_reasoning_tools = val;
    await persist();
  });

  // 9. Auto-Update Checks
  bindSwitch('toggle-gen-auto-update', currentSettings.auto_update_checks, async (val) => {
    currentSettings.auto_update_checks = val;
    await persist();
  });

  // 10. Telemetry & Diagnostics
  bindSwitch('toggle-gen-telemetry', currentSettings.telemetry_enabled, async (val) => {
    currentSettings.telemetry_enabled = val;
    await persist();
  });
}

/**
 * Binds segmented choice selectors (e.g. Close Action choice).
 */
function setupSegmentedChoices(): void {
  const closeActionButtons = document.querySelectorAll<HTMLButtonElement>('.seg-choice-close-action');
  closeActionButtons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      if (btn.disabled || btn.classList.contains('disabled')) return;
      const idx = parseInt(btn.dataset.index || '0', 10);
      currentSettings.close_button_action = idx;
      updateSegmentedChoiceUI('.seg-choice-close-action', idx);
      await persist();
    });
  });
}

/**
 * Helper to bind a standard toggle switch element.
 */
function bindSwitch(id: string, initial: boolean, onToggle: (checked: boolean) => Promise<void>): void {
  const switchEl = document.getElementById(id);
  if (!switchEl) return;

  switchEl.classList.toggle('checked', initial);
  switchEl.setAttribute('aria-checked', String(initial));

  switchEl.addEventListener('click', async () => {
    if (switchEl.classList.contains('disabled')) return;
    const isChecked = switchEl.classList.toggle('checked');
    switchEl.setAttribute('aria-checked', String(isChecked));
    await onToggle(isChecked);
  });
}

/**
 * Updates segmented choice active highlight.
 */
function updateSegmentedChoiceUI(selector: string, selectedIndex: number): void {
  const buttons = document.querySelectorAll<HTMLButtonElement>(selector);
  buttons.forEach((btn) => {
    const idx = parseInt(btn.dataset.index || '0', 10);
    btn.classList.toggle('active', idx === selectedIndex);
  });
}

/**
 * Synchronizes DOM state with currentSettings and applies dependent state locking.
 */
function syncGeneralUI(): void {
  const trayEnabled = currentSettings.minimize_to_tray_enabled;

  setSwitchChecked('toggle-gen-autostart', currentSettings.autostart_enabled);
  setSwitchChecked('toggle-gen-tray', trayEnabled);
  setSwitchChecked('toggle-gen-start-min', currentSettings.start_minimized);
  setElementDisabled('toggle-gen-start-min', !trayEnabled);

  setSwitchChecked('toggle-gen-auto-approve', currentSettings.global_auto_approve_default);
  setSwitchChecked('toggle-gen-auto-scroll', currentSettings.auto_scroll_stream);
  setSwitchChecked('toggle-gen-notify-perm', currentSettings.notify_on_permission_request);
  setSwitchChecked('toggle-gen-notify-complete', currentSettings.notify_on_response_complete);
  setSwitchChecked('toggle-gen-auto-collapse', currentSettings.auto_collapse_reasoning_tools);
  setSwitchChecked('toggle-gen-auto-update', currentSettings.auto_update_checks);
  setSwitchChecked('toggle-gen-telemetry', currentSettings.telemetry_enabled);

  updateSegmentedChoiceUI('.seg-choice-close-action', currentSettings.close_button_action);

  // Lock "Minimize to Tray" choice button if tray is disabled
  const closeToTrayBtn = document.querySelector<HTMLButtonElement>('.seg-choice-close-action[data-index="1"]');
  if (closeToTrayBtn) {
    closeToTrayBtn.disabled = !trayEnabled;
    closeToTrayBtn.classList.toggle('disabled', !trayEnabled);
  }
}

function setSwitchChecked(id: string, checked: boolean): void {
  const el = document.getElementById(id);
  if (el) {
    el.classList.toggle('checked', checked);
    el.setAttribute('aria-checked', String(checked));
  }
}

function setElementDisabled(id: string, disabled: boolean): void {
  const el = document.getElementById(id);
  if (el) {
    el.classList.toggle('disabled', disabled);
    el.setAttribute('aria-disabled', String(disabled));
  }
}

async function persist(): Promise<void> {
  try {
    await saveGeneralSettingsIpc(currentSettings);
  } catch (err) {
    console.error('[GeneralSettings] Persist failed:', err);
  }
}
