// General Settings Controller & DOM Coordinator for VS Code
//
// Manages:
// - Section 1: Session & Chat Defaults (Auto-Approve, Auto-Scroll, Permission Notifications, Finish Notifications, Auto-Collapse Cards)
// - Section 2: Updates & Diagnostics (Auto Update Checks, Anonymous Diagnostics)

import { listenIpcEvent } from '../../shared/ipc.js';
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
  syncGeneralUI();

  // Listen to external toggle changes (e.g. from the input panel shield button)
  listenIpcEvent<boolean>('operon://auto-approve-changed', (enabled) => {
    currentSettings.global_auto_approve_default = enabled;
    syncGeneralUI();
  });
}

/**
 * Binds all toggle switch controls in the General settings panel.
 */
function setupToggleSwitches(): void {
  // 1. Auto-Approve Default
  bindSwitch('toggle-gen-auto-approve', currentSettings.global_auto_approve_default, async (val) => {
    currentSettings.global_auto_approve_default = val;
    await persist();
  });

  // 2. Auto-Scroll Stream
  bindSwitch('toggle-gen-auto-scroll', currentSettings.auto_scroll_stream, async (val) => {
    currentSettings.auto_scroll_stream = val;
    await persist();
  });

  // 3. Notify on Permission
  bindSwitch('toggle-gen-notify-perm', currentSettings.notify_on_permission_request, async (val) => {
    currentSettings.notify_on_permission_request = val;
    await persist();
  });

  // 4. Notify on Response Complete
  bindSwitch('toggle-gen-notify-complete', currentSettings.notify_on_response_complete, async (val) => {
    currentSettings.notify_on_response_complete = val;
    await persist();
  });

  // 5. Auto-Collapse Reasoning & Tool Cards
  bindSwitch('toggle-gen-auto-collapse', currentSettings.auto_collapse_reasoning_tools, async (val) => {
    currentSettings.auto_collapse_reasoning_tools = val;
    await persist();
  });

  // 6. Auto-Update Checks
  bindSwitch('toggle-gen-auto-update', currentSettings.auto_update_checks, async (val) => {
    currentSettings.auto_update_checks = val;
    await persist();
  });

  // 7. Telemetry & Diagnostics
  bindSwitch('toggle-gen-telemetry', currentSettings.telemetry_enabled, async (val) => {
    currentSettings.telemetry_enabled = val;
    await persist();
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
 * Synchronizes DOM state with currentSettings.
 */
function syncGeneralUI(): void {
  setSwitchChecked('toggle-gen-auto-approve', currentSettings.global_auto_approve_default);
  setSwitchChecked('toggle-gen-auto-scroll', currentSettings.auto_scroll_stream);
  setSwitchChecked('toggle-gen-notify-perm', currentSettings.notify_on_permission_request);
  setSwitchChecked('toggle-gen-notify-complete', currentSettings.notify_on_response_complete);
  setSwitchChecked('toggle-gen-auto-collapse', currentSettings.auto_collapse_reasoning_tools);
  setSwitchChecked('toggle-gen-auto-update', currentSettings.auto_update_checks);
  setSwitchChecked('toggle-gen-telemetry', currentSettings.telemetry_enabled);
}

function setSwitchChecked(id: string, checked: boolean): void {
  const el = document.getElementById(id);
  if (el) {
    el.classList.toggle('checked', checked);
    el.setAttribute('aria-checked', String(checked));
  }
}

async function persist(): Promise<void> {
  try {
    await saveGeneralSettingsIpc(currentSettings);
  } catch (err) {
    console.error('[GeneralSettings] Persist failed:', err);
  }
}
