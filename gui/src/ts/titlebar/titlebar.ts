// Titlebar component entrypoint

import { setupBrandLogoToggle, setupWindowActions } from './actions.js';
import { setupMenus } from './menu.js';
import { setupNavigationControls } from './navigation.js';

export function initTitlebar(): void {
  setupBrandLogoToggle();
  setupNavigationControls();
  setupMenus();
  setupWindowActions();
}
