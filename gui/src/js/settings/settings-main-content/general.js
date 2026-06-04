'use strict';

/**
 * general.js
 *
 * General settings page for the Operon settings panel.
 *
 * Covers:
 *  - App update check button
 *  - Launch at startup toggle
 *  - Enter key behavior selector
 *  - Auto-scroll toggle
 *  - Code wrap toggle
 *  - Language selector
 *  - Keyboard shortcuts button (placeholder)
 *  - Reset to defaults button
 *  - About section at the bottom
 *
 * All settings are read from / saved to the central store.
 */

import { showSuccess, showError } from '../../shared/toast.js';
import { buildSettingRow, escapeHtml } from '../settings-panel.js';

// ── Local Settings State (Placeholder UI) ────────────────────────────────────

/**
 * In-memory settings for the placeholder UI.
 */
const localSettings = {
  launchAtStartup: false,
  enterBehavior: 'send',
  autoScroll: true,
  codeWrap: false,
  language: 'en',
};

// ── Page Builder ─────────────────────────────────────────────────────────────

/**
 * Builds the full HTML for the General settings page.
 *
 * @param {Object} settings - User settings object from the store.
 * @param {Object} state    - Full app state.
 * @returns {string} HTML string.
 */
function buildGeneralPage(settings) {
  // Read current setting values from local settings with safe defaults
  const launchAtStartup = Boolean(localSettings.launchAtStartup);
  const enterBehavior   = localSettings.enterBehavior || 'send';
  const autoScroll      = localSettings.autoScroll !== false;    // default true
  const codeWrap        = Boolean(localSettings.codeWrap);
  const language        = localSettings.language || 'en';

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
        `<label class="setting-toggle">
           <input type="checkbox" id="setting-launch-startup" ${launchAtStartup ? 'checked' : ''}>
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

      <!-- ── Auto-Scroll ───────────────────────────────────────────────── -->
      ${buildSettingRow(
        'Auto-scroll',
        'Automatically scroll to the latest message',
        `<label class="setting-toggle">
           <input type="checkbox" id="setting-auto-scroll" ${autoScroll ? 'checked' : ''}>
           <span class="setting-toggle__slider"></span>
         </label>`
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

      <!-- ── Language ──────────────────────────────────────────────────── -->
      ${buildSettingRow(
        'Language',
        'Select your preferred display language',
        `<select class="setting-select" id="setting-language">
           <option value="en" ${language === 'en' ? 'selected' : ''}>English</option>
           <option value="es" ${language === 'es' ? 'selected' : ''}>Spanish</option>
           <option value="fr" ${language === 'fr' ? 'selected' : ''}>French</option>
           <option value="de" ${language === 'de' ? 'selected' : ''}>German</option>
           <option value="zh" ${language === 'zh' ? 'selected' : ''}>Chinese</option>
           <option value="ja" ${language === 'ja' ? 'selected' : ''}>Japanese</option>
           <option value="pt" ${language === 'pt' ? 'selected' : ''}>Portuguese</option>
         </select>`
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
 * Called when the user interacts with controls that have immediate effect,
 * or from the "Check for updates" button handler in settings-panel.js.
 */
function saveSettings() {
  // Read each control's current value from the DOM
  const autoScrollEl     = /** @type {HTMLInputElement|null}  */ (document.getElementById('setting-auto-scroll'));
  const codeWrapEl       = /** @type {HTMLInputElement|null}  */ (document.getElementById('setting-code-wrap'));
  const launchStartupEl  = /** @type {HTMLInputElement|null}  */ (document.getElementById('setting-launch-startup'));
  const enterBehaviorEl  = /** @type {HTMLSelectElement|null} */ (document.getElementById('setting-enter-behavior'));
  const languageEl       = /** @type {HTMLSelectElement|null} */ (document.getElementById('setting-language'));

  // Merge new values into local settings
  if (autoScrollEl)    localSettings.autoScroll      = autoScrollEl.checked;
  if (codeWrapEl)      localSettings.codeWrap         = codeWrapEl.checked;
  if (launchStartupEl) localSettings.launchAtStartup  = launchStartupEl.checked;
  if (enterBehaviorEl) localSettings.enterBehavior    = enterBehaviorEl.value;
  if (languageEl)      localSettings.language         = languageEl.value;

  showSuccess('Settings saved');
}

/**
 * Resets all General settings to their default values and shows a toast.
 */
function resetToDefaults() {
  const defaults = {
    launchAtStartup: false,
    enterBehavior: 'send',
    autoScroll: true,
    codeWrap: false,
    language: 'en',
  };

  // Reset to defaults
  Object.assign(localSettings, defaults);
  showSuccess('Settings reset to defaults');
}

// ── Hydration (Event Binding) ─────────────────────────────────────────────────

/**
 * Attaches DOM event listeners to the General settings page controls.
 * Call this after injecting the page HTML into the DOM.
 *
 * @param {HTMLElement} container - The settings main content element.
 */
function hydrateGeneralPage(container) {
  if (!container) return;

  // Auto-save on each control change — no explicit Save button needed
  const autoSaveInputs = [
    '#setting-auto-scroll',
    '#setting-code-wrap',
    '#setting-launch-startup',
    '#setting-enter-behavior',
    '#setting-language',
  ];

  autoSaveInputs.forEach(selector => {
    container.querySelector(selector)?.addEventListener('change', saveSettings);
  });

  // "Check for updates" — wired externally by settings-panel.js but
  // bind here too as a safety net
  container.querySelector('#btn-check-updates')?.addEventListener('click', () => {
    // TODO: call ipc.checkForUpdates() when available
    showSuccess('Checking for updates...');
  });

  // Reset all settings to defaults
  container.querySelector('#btn-reset-defaults')?.addEventListener('click', () => {
    resetToDefaults();
  });
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { buildGeneralPage, saveSettings, hydrateGeneralPage };
