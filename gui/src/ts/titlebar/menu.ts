import { createNewSessionIpc, openProjectPickerIpc } from '../left-sidebar/ipc.js';
import { refreshSidebarContent } from '../left-sidebar/sidebar.js';
import { sidebarState } from '../left-sidebar/state.js';
import { rightSidebarState } from '../right-sidebar/state.js';
import { openSettingsWindowIpc } from '../settings/ipc.js';
import { invokeIpc } from '../shared/ipc.js';
import { appState } from '../shared/state.js';

export function setupMenus(): void {
  const menuTriggers = document.querySelectorAll<HTMLButtonElement>('.menu-trigger');
  const dropdownMenus = document.querySelectorAll<HTMLElement>('.dropdown-menu');

  // Open/toggle dropdown when menu trigger is clicked
  menuTriggers.forEach((trigger) => {
    trigger.addEventListener('click', (e) => {
      e.stopPropagation();
      const menuName = trigger.dataset.menu || null;
      const current = appState.getActiveMenu();
      appState.setActiveMenu(current === menuName ? null : menuName);
    });

    // Hover-switch when a menu is already open
    trigger.addEventListener('mouseenter', () => {
      const current = appState.getActiveMenu();
      if (current && trigger.dataset.menu) {
        appState.setActiveMenu(trigger.dataset.menu);
      }
    });
  });

  // Sync active menu state with DOM classes
  appState.subscribe(() => {
    const active = appState.getActiveMenu();

    menuTriggers.forEach((btn) => {
      btn.classList.toggle('active', btn.dataset.menu === active);
    });

    dropdownMenus.forEach((menu) => {
      menu.classList.toggle('open', menu.dataset.menu === active);
    });
  });

  // Dismiss menus on outside click
  window.addEventListener('click', () => {
    if (appState.getActiveMenu()) {
      appState.setActiveMenu(null);
    }
  });

  // Wire specific menu actions
  setupFilesMenuActions();
  setupViewMenuActions();
  setupWindowMenuActions();
  setupHelpMenuActions();
}

function setupFilesMenuActions(): void {
  document.getElementById('menu-item-new-chat')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    const newId = await createNewSessionIpc(undefined, undefined);
    sidebarState.selectSession(newId, null);
    await refreshSidebarContent();
  });

  document.getElementById('menu-item-open-project')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    const picked = await openProjectPickerIpc();
    if (picked) {
      sidebarState.setActiveProjectPath(picked);
      await refreshSidebarContent();
    }
  });

  document.getElementById('menu-item-settings')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await openSettingsWindowIpc();
  });
}

function setupViewMenuActions(): void {
  document.getElementById('menu-item-toggle-sidebar')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    const newState = await invokeIpc<boolean>('toggle_sidebar');
    if (newState !== null) {
      appState.setSidebarOpen(newState);
    } else {
      appState.toggleSidebar();
    }
  });

  document.getElementById('menu-item-toggle-terminal')?.addEventListener('click', () => {
    appState.setActiveMenu(null);
    console.debug('[Menu:View] Toggle Terminal requested');
  });

  document.getElementById('menu-item-toggle-git-diff')?.addEventListener('click', () => {
    appState.setActiveMenu(null);
    rightSidebarState.toggleOpen();
  });
}

function setupWindowMenuActions(): void {
  document.getElementById('menu-item-close-window')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await invokeIpc('close_window');
  });

  document.getElementById('menu-item-exit')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await invokeIpc('exit_application');
  });
}

function setupHelpMenuActions(): void {
  document.getElementById('menu-item-documentation')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await invokeIpc('open_documentation');
  });

  document.getElementById('menu-item-report-bug')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await invokeIpc('open_report_bug');
  });

  document.getElementById('menu-item-follow-creator')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await invokeIpc('open_follow_creator');
  });

  document.getElementById('menu-item-see-repo')?.addEventListener('click', async () => {
    appState.setActiveMenu(null);
    await invokeIpc('open_repository');
  });
}
