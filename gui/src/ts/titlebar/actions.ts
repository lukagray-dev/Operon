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

  // Check initial maximized state
  invokeIpc<boolean>('is_window_maximized').then((isMax) => {
    if (isMax !== null) {
      appState.setIsMaximized(isMax);
    }
  });

  // Update maximize icon class on state changes
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

  // Setup dragging
  setupWindowDragging();
}

export function setupWindowDragging(): void {
  const titlebar = document.getElementById('app-titlebar');
  const dragSpacer = document.querySelector('.titlebar-drag-spacer');

  // Explicit mousedown handler fallback for dragging
  const handleDrag = async (e: MouseEvent) => {
    // Only drag on left-click and when not clicking an interactive button or menu
    if (e.button === 0) {
      const target = e.target as HTMLElement | null;
      if (
        target === titlebar ||
        target === dragSpacer ||
        target?.classList.contains('titlebar-left') ||
        target?.hasAttribute('data-tauri-drag-region')
      ) {
        try {
          await invokeIpc('start_dragging');
        } catch {
          // Native data-tauri-drag-region or window drag handled
        }
      }
    }
  };

  titlebar?.addEventListener('mousedown', handleDrag);

  // Double click to maximize / restore
  titlebar?.addEventListener('dblclick', async (e) => {
    const target = e.target as HTMLElement | null;
    if (
      target === titlebar ||
      target === dragSpacer ||
      target?.classList.contains('titlebar-left') ||
      target?.hasAttribute('data-tauri-drag-region')
    ) {
      const isMax = await invokeIpc<boolean>('toggle_maximize_window');
      if (isMax !== null) {
        appState.setIsMaximized(isMax);
      }
    }
  });
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
