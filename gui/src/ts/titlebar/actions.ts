// Window action controls: minimize, maximize, close, dragging, and brand sidebar toggle

import { invokeIpc } from '../shared/ipc.js';
import { appState } from '../shared/state.js';

export function setupWindowActions(): void {
  const minBtn = document.getElementById('btn-minimize');
  const maxBtn = document.getElementById('btn-maximize');
  const closeBtn = document.getElementById('btn-close');
  const maxIcon = document.getElementById('icon-max-restore');

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
      appState.setIsMaximized(isMax);
    }
  });

  // Close
  closeBtn?.addEventListener('click', async (e) => {
    e.stopPropagation();
    await invokeIpc('close_window');
  });

  // Initial synchronization: query Rust backend to check if the window launched maximized
  invokeIpc<boolean>('is_window_maximized').then((isMax) => {
    if (isMax !== null) {
      appState.setIsMaximized(isMax);
    }
  });

  // Keep maximize icon synchronized whenever the app state changes
  appState.subscribe(() => {
    if (maxIcon) {
      const isMax = appState.getIsMaximized();
      if (isMax) {
        maxIcon.classList.remove('icon-maximize');
        maxIcon.classList.add('icon-unmaxmize');
      } else {
        maxIcon.classList.remove('icon-unmaxmize');
        maxIcon.classList.add('icon-maximize');
      }
    }
  });

  // Listen to window resize events to keep the maximize/restore icon and state synchronized.
  // Whenever the user double-clicks the native titlebar, uses Aero Snap, or presses Win+Up / Win+Down,
  // the OS resizes the window, firing the DOM 'resize' event, where we query the active maximized state.
  window.addEventListener('resize', async () => {
    const isMax = await invokeIpc<boolean>('is_window_maximized');
    if (isMax !== null) {
      appState.setIsMaximized(isMax);
    }
  });

  // Setup dragging support for the titlebar
  setupWindowDragging();
}

/**
 * Sets up titlebar dragging and double-click maximization capabilities.
 *
 * HOW TAURI / WINDOWS WINDOW MANAGEMENT WORKS:
 * 1. When the titlebar is clicked once (e.detail === 1), we invoke 'start_dragging'
 *    so the OS handles native smooth window movement across multi-monitors and screen edges.
 * 2. When the user double-clicks (e.detail === 2 or any even click), we invoke 'toggle_maximize_window'
 *    directly, and we intentionally do NOT call 'start_dragging'. If 'start_dragging' were called on
 *    the second click of a double click, Windows OS would interpret the drag on a maximized window as
 *    an Aero Snap tear-off, immediately unmaximizing it!
 * 3. By differentiating single-click (drag) vs double-click (maximize) via e.detail, the window
 *    maximizes cleanly and stays maximized without bouncing back.
 */
export function setupWindowDragging(): void {
  const titlebar = document.getElementById('app-titlebar');
  const dragSpacer = document.querySelector('.titlebar-drag-spacer');

  const handleMousedown = async (e: MouseEvent) => {
    // Only process primary left-click (button 0)
    if (e.button === 0) {
      const target = e.target as HTMLElement | null;

      // Do NOT initiate window drag or toggle maximize if the user clicked inside interactive UI elements
      // like buttons, dropdown menus, or brand sidebar toggles.
      if (
        target?.closest('button') ||
        target?.closest('.dropdown-menu') ||
        target?.closest('.brand-container')
      ) {
        return;
      }

      // Verify the click occurred on a valid draggable titlebar region
      if (
        target === titlebar ||
        target === dragSpacer ||
        target?.classList.contains('titlebar-left') ||
        target?.classList.contains('titlebar') ||
        target?.closest('.titlebar-left')
      ) {
        if (e.detail % 2 === 0) {
          // Double-click (or 4th click, etc.): Toggle maximize / restore
          const isMax = await invokeIpc<boolean>('toggle_maximize_window');
          if (isMax !== null) {
            appState.setIsMaximized(isMax);
          }
        } else {
          // Single-click (or 3rd click, etc.): Start window dragging
          try {
            await invokeIpc('start_dragging');
          } catch {
            // Window drag error handled gracefully
          }
        }
      }
    }
  };

  titlebar?.addEventListener('mousedown', handleMousedown);
}

export function setupBrandLogoToggle(): void {
  const brandContainer = document.getElementById('brand-container');
  const brandToggleIcon = document.getElementById('brand-toggle-icon');

  if (!brandContainer || !brandToggleIcon) return;

  const updateToggleIcon = () => {
    const isOpen = appState.getSidebarOpen();
    const iconUrl = isOpen
      ? 'url("assets/icons/titlebar/sidebar-opened.svg")'
      : 'url("assets/icons/titlebar/sidebar-closed.svg")';
    brandToggleIcon.style.webkitMaskImage = iconUrl;
    brandToggleIcon.style.maskImage = iconUrl;
  };

  brandContainer.addEventListener('mouseenter', () => {
    updateToggleIcon();
    brandContainer.classList.add('hovered');
  });

  brandContainer.addEventListener('mouseleave', () => {
    brandContainer.classList.remove('hovered');
  });

  brandContainer.addEventListener('click', async (e) => {
    e.stopPropagation();
    const newState = await invokeIpc<boolean>('toggle_sidebar');
    if (newState !== null) {
      appState.setSidebarOpen(newState);
    } else {
      appState.toggleSidebar();
    }
    updateToggleIcon();
  });
}
