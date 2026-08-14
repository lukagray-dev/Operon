// Application root coordinator

import { initSidebar } from './left-sidebar/sidebar.js';
import { initInputPanel } from './main-content/input/input.js';
import { initTitlebar } from './titlebar/titlebar.js';

window.addEventListener('DOMContentLoaded', () => {
  // Initialize Titlebar
  initTitlebar();

  // Initialize Left Sidebar
  initSidebar();

  // Initialize Main Content Input Panel
  initInputPanel();

  console.debug('[Operon GUI] Initialized with static TypeScript architecture.');
});
