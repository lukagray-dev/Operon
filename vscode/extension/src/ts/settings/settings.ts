// ============================================================================
// Settings Window Root Coordinator
//
// Hey friend! This is the root coordinator for the Settings Tab.
// It initializes all settings sub-controllers:
// 1. Sidebar Category Navigation & Search
// 2. General Preferences Panel
// 3. Appearance, Code & Table Themes, Orb Selection, Typography
// 4. Models & Providers list + setup form view
// 5. Permissions Allowed Directories & Global Tools
// 6. Channels (WhatsApp & Telegram)
// 7. Memory Management
// 8. About System Specifications & Links
// ============================================================================

import { initAboutSettings } from './about/about.js';
import { initAppearanceSettings } from './appearance/appearance.js';
import { initGeneralSettings } from './general/general.js';
import { initMemorySettings } from './memory/memory.js';
import { initModelsSettings } from './models/models.js';
import { initPermissionsSettings } from './permissions/permissions.js';
import { initSettingsSidebar } from './sidebar/sidebar.js';

async function initSettings(): Promise<void> {
  console.log('[Operon Settings] Initializing all settings modules...');

  // 1. Initialize Sidebar Category Navigation immediately
  try {
    initSettingsSidebar();
  } catch (err) {
    console.error('[Operon Settings] Error initializing sidebar:', err);
  }

  // 2. Initialize Panels with error isolation
  try {
    await initGeneralSettings();
  } catch (err) {
    console.error('[Operon Settings] Error initializing General settings:', err);
  }

  try {
    await initAppearanceSettings();
  } catch (err) {
    console.error('[Operon Settings] Error initializing Appearance settings:', err);
  }

  try {
    await initModelsSettings();
  } catch (err) {
    console.error('[Operon Settings] Error initializing Models settings:', err);
  }

  try {
    await initPermissionsSettings();
  } catch (err) {
    console.error('[Operon Settings] Error initializing Permissions settings:', err);
  }

  try {
    await initMemorySettings();
  } catch (err) {
    console.error('[Operon Settings] Error initializing Memory settings:', err);
  }

  try {
    await initAboutSettings();
  } catch (err) {
    console.error('[Operon Settings] Error initializing About settings:', err);
  }

  console.log('[Operon Settings] All settings panels initialized successfully.');
}

if (document.readyState === 'loading') {
  window.addEventListener('DOMContentLoaded', initSettings);
} else {
  initSettings();
}
