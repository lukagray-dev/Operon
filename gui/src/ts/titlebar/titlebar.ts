// Titlebar component entrypoint

import { setupBrandLogoToggle, setupWindowActions } from './actions.js';
import { setupMenus } from './menu.js';


export function initTitlebar(): void {
  setupBrandLogoToggle();

  setupMenus();
  setupWindowActions();
}
