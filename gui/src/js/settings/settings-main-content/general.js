'use strict';

/**
 * general.js
 *
 * General settings page for the Operon settings panel.
 *
 * Covers:
 *  - App update check button (disabled / coming soon)
 *  - Launch at startup toggle (disabled / coming soon)
 *  - Enter key behavior selector
 *  - Code wrap toggle
 *  - Reset to defaults button
 *  - About section at the bottom
 *
 * All settings are read from / saved to localStorage.
 */

import { showSuccess } from '../../shared/toast.js';
import { buildSettingRow } from '../settings-panel.js';

// ── Local Settings State (localStorage Persistent) ───────────────────────────

const DEFAULT_SETTINGS = {
  launchAtStartup: false,
  enterBehavior: 'send',
  autoScroll: true,
  codeWrap: false,
};

let localSettings = { ...DEFAULT_SETTINGS };

// Load initially from localStorage
try {
  const stored = localStorage.getItem('operon-settings-general');
  if (stored) {
    Object.assign(localSettings, JSON.parse(stored));
  }
} catch (e) {
  console.error('Failed to load general settings:', e);
}

// ── Page Builder ─────────────────────────────────────────────────────────────

/**
 * Builds the full HTML for the General settings page.
 *
 * @param {Object} settings - User settings object from the store.
 * @returns {string} HTML string.
 */
function buildGeneralPage(settings) {
  const enterBehavior = localSettings.enterBehavior || 'send';
  const codeWrap      = Boolean(localSettings.codeWrap);

  return `
    <div class="settings-page settings-general">
      <h2 class="settings-page__title">General</h2>
      <p class="settings-page__description">Configure app behavior and preferences</p>

      <!-- ── App Updates ───────────────────────────────────────────────── -->
      ${buildSettingRow(
        'App updates',
        'Check for new versions and updates',
        `<button class="btn btn--secondary btn--sm" id="btn-check-updates" type="button">
           Check for updates
         </button>`
      )}

      <!-- ── Launch at Startup ─────────────────────────────────────────── -->
      ${buildSettingRow(
        'Launch at startup',
        'Start Operon automatically when you log in',
        `<label class="setting-toggle" style="opacity: 0.6; cursor: not-allowed;">
           <input type="checkbox" id="setting-launch-startup" disabled>
           <span class="setting-toggle__slider"></span>
         </label>`
      )}

      <!-- ── Enter Key Behavior ────────────────────────────────────────── -->
      ${buildSettingRow(
        'Enter key behavior',
        'Choose what the Enter key does in the message box',
        `<select class="setting-select" id="setting-enter-behavior">
           <option value="send"    ${enterBehavior === 'send'    ? 'selected' : ''}>Send message</option>
           <option value="newline" ${enterBehavior === 'newline' ? 'selected' : ''}>Insert new line</option>
         </select>`
      )}

      <!-- ── Code Wrap ─────────────────────────────────────────────────── -->
      ${buildSettingRow(
        'Code wrap',
        'Wrap long lines inside code blocks',
        `<label class="setting-toggle">
           <input type="checkbox" id="setting-code-wrap" ${codeWrap ? 'checked' : ''}>
           <span class="setting-toggle__slider"></span>
         </label>`
      )}

      <!-- ── Reset to Defaults ─────────────────────────────────────────── -->
      ${buildSettingRow(
        'Reset to defaults',
        'Restore all settings to their original factory values',
        `<button class="btn btn--danger btn--sm" id="btn-reset-defaults" type="button">
           Reset
         </button>`
      )}
    </div>
  `;
}

// ── Settings Persistence ─────────────────────────────────────────────────────

/**
 * Reads all General settings controls from the DOM and saves them to the store.
 */
function saveSettings() {
  const codeWrapEl      = /** @type {HTMLInputElement|null}  */ (document.getElementById('setting-code-wrap'));
  const enterBehaviorEl = /** @type {HTMLSelectElement|null} */ (document.getElementById('setting-enter-behavior'));

  if (codeWrapEl)      localSettings.codeWrap      = codeWrapEl.checked;
  if (enterBehaviorEl) localSettings.enterBehavior = enterBehaviorEl.value;

  try {
    localStorage.setItem('operon-settings-general', JSON.stringify(localSettings));
  } catch (e) {
    console.error('Failed to save general settings:', e);
  }

  showSuccess('Settings saved');
}

/**
 * Resets all General settings to their default values and shows a toast.
 */
function resetToDefaults() {
  localSettings = { ...DEFAULT_SETTINGS };
  try {
    localStorage.setItem('operon-settings-general', JSON.stringify(localSettings));
  } catch (e) {
    console.error('Failed to save general settings:', e);
  }

  // Update UI values immediately if they exist in DOM
  const codeWrapEl      = /** @type {HTMLInputElement|null}  */ (document.getElementById('setting-code-wrap'));
  const enterBehaviorEl = /** @type {HTMLSelectElement|null} */ (document.getElementById('setting-enter-behavior'));
  
  if (codeWrapEl)      codeWrapEl.checked = localSettings.codeWrap;
  if (enterBehaviorEl) enterBehaviorEl.value = localSettings.enterBehavior;

  showSuccess('Settings reset to defaults');
}

// ── Hydration (Event Binding) ─────────────────────────────────────────────────

/**
 * Attaches DOM event listeners to the General settings page controls.
 *
 * @param {HTMLElement} container - The settings main content element.
 */
function hydrateGeneralPage(container) {
  if (!container) return;

  // Auto-save on each control change
  const autoSaveInputs = [
    '#setting-code-wrap',
    '#setting-enter-behavior',
  ];

  autoSaveInputs.forEach(selector => {
    container.querySelector(selector)?.addEventListener('change', saveSettings);
  });

  // "Check for updates" dialogue
  container.querySelector('#btn-check-updates')?.addEventListener('click', (e) => {
    e.preventDefault();
    e.stopPropagation();
    alert('This functionality is coming soon.');
  });

  // "Launch at startup" dialogue
  const launchStartupEl = container.querySelector('#setting-launch-startup');
  if (launchStartupEl) {
    const parentToggle = launchStartupEl.closest('.setting-toggle');
    if (parentToggle) {
      parentToggle.addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        alert('Launch at startup feature is coming soon!');
      });
    }
  }

  // Reset all settings to defaults
  container.querySelector('#btn-reset-defaults')?.addEventListener('click', () => {
    resetToDefaults();
  });
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { buildGeneralPage, saveSettings, hydrateGeneralPage };
