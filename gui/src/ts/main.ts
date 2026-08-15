// Application root coordinator

import { initSidebar } from './left-sidebar/sidebar.js';
import { initEmptyState } from './main-content/empty-state/empty-state.js';
import { initInputPanel } from './main-content/input/input.js';
import { initMessages } from './main-content/messages/messages.js';
import { initTopbar } from './main-content/topbar/topbar.js';
import { initRightSidebar } from './right-sidebar/right-sidebar.js';
import { initTitlebar } from './titlebar/titlebar.js';

window.addEventListener('DOMContentLoaded', () => {
  // Initialize Titlebar
  initTitlebar();

  // Initialize Left Sidebar
  initSidebar();

  // Initialize Main Content Topbar
  initTopbar();

  // Initialize Empty State
  initEmptyState();

  // Initialize Chat Messages Stream
  initMessages();

  // Initialize Main Content Input Panel
  initInputPanel();

  // Initialize Right Sidebar (Source Control & Git Diff)
  initRightSidebar();

  console.debug('[Operon GUI] Initialized with static TypeScript architecture.');
});
