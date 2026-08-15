// Terminal Controller & xterm.js Manager
//
// Manages the bottom-docked interactive PowerShell terminal panel:
// - Multi-tab terminal management with add/close controls.
// - xterm.js terminal emulator with Kode Mono font rendering.
// - Vertical drag-to-resize with localStorage persistence.
// - Context-aware working directory resolution (project root in project sessions,
//   global default workspace in general chat sessions).

import { sidebarState } from '../../left-sidebar/state.js';
import {
  closeTerminalIpc,
  createTerminalIpc,
  getTerminalDefaultWorkdirIpc,
  listenTerminalClosed,
  listenTerminalOutput,
  resizeTerminalIpc,
  writeTerminalIpc,
} from './ipc.js';
import { terminalState } from './state.js';
import type { TerminalTab, XtermFitAddon, XtermTerminal } from './types.js';

declare global {
  interface Window {
    Terminal?: new (options?: Record<string, unknown>) => XtermTerminal;
    FitAddon?: {
      FitAddon: new () => XtermFitAddon;
    };
  }
}

let panelEl: HTMLElement | null = null;
let toggleBtnEl: HTMLElement | null = null;
let tabsContainerEl: HTMLElement | null = null;
let addTabBtnEl: HTMLElement | null = null;
let terminalContainerEl: HTMLElement | null = null;
let resizeHandleEl: HTMLElement | null = null;

let isResizing = false;

/**
 * Initializes the terminal panel, registers event listeners, and hooks up keybindings.
 */
export async function initTerminal(): Promise<void> {
  panelEl = document.getElementById('terminal-panel');
  toggleBtnEl = document.getElementById('btn-topbar-terminal');
  tabsContainerEl = document.getElementById('terminal-tabs-container');
  addTabBtnEl = document.getElementById('terminal-add-tab-btn');
  terminalContainerEl = document.getElementById('terminal-container');
  resizeHandleEl = document.getElementById('terminal-resize-handle');

  if (!panelEl || !toggleBtnEl || !terminalContainerEl) {
    console.warn('[Terminal] Essential DOM elements not found.');
    return;
  }

  // 1. Topbar toggle button click
  toggleBtnEl.addEventListener('click', () => {
    toggleTerminalPanel();
  });

  // 2. View menu toggle terminal
  const menuItemToggle = document.getElementById('menu-item-toggle-terminal');
  if (menuItemToggle) {
    menuItemToggle.addEventListener('click', () => {
      toggleTerminalPanel();
    });
  }

  // 3. Add tab button
  if (addTabBtnEl) {
    addTabBtnEl.addEventListener('click', () => {
      createNewTerminalTab();
    });
  }

  // 5. Drag to resize
  if (resizeHandleEl) {
    setupDragResize(resizeHandleEl);
  }

  // 6. Global keyboard shortcut: Ctrl+`
  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === '`') {
      e.preventDefault();
      toggleTerminalPanel();
    }
  });

  // 7. Window resize fitting
  window.addEventListener('resize', () => {
    fitActiveTerminal();
  });

  // 8. Backend event listeners for streaming output and process termination
  await listenTerminalOutput((payload) => {
    const tab = terminalState.getTabs().find((t) => t.id === payload.id);
    if (tab && tab.term) {
      tab.term.write(payload.data);
    }
  });

  await listenTerminalClosed((payload) => {
    handleTerminalProcessExited(payload.id);
  });

  // 9. ResizeObserver to keep active terminal perfectly sized on all panel/window changes
  if (terminalContainerEl && typeof ResizeObserver !== 'undefined') {
    const ro = new ResizeObserver(() => {
      if (terminalState.isOpen()) {
        fitActiveTerminal();
      }
    });
    ro.observe(terminalContainerEl);
  }

  // 10. Subscribe to state changes to update tab bar UI
  terminalState.subscribe(() => {
    renderTabs();
  });

  console.log('[Terminal] Initialized successfully.');
}

/**
 * Toggles the visibility of the bottom terminal panel.
 */
export async function toggleTerminalPanel(): Promise<void> {
  if (terminalState.isOpen()) {
    closeTerminalPanel();
  } else {
    await openTerminalPanel();
  }
}

/**
 * Opens the terminal panel, restoring saved height and creating a default tab if none exists.
 */
export async function openTerminalPanel(): Promise<void> {
  if (!panelEl || !toggleBtnEl) return;

  terminalState.setOpen(true);
  panelEl.classList.remove('collapsed');
  toggleBtnEl.classList.add('active');

  const height = terminalState.getSavedHeight();
  document.documentElement.style.setProperty('--terminal-height', `${height}px`);

  // If no active tabs exist, spawn the first session
  if (terminalState.getTabs().length === 0) {
    await createNewTerminalTab();
  } else {
    requestAnimationFrame(() => {
      fitActiveTerminal();
      focusActiveTerminal();
    });
  }
}

/**
 * Collapses the terminal panel and resets layout variables.
 */
export function closeTerminalPanel(): void {
  if (!panelEl || !toggleBtnEl) return;

  terminalState.setOpen(false);
  panelEl.classList.add('collapsed');
  toggleBtnEl.classList.remove('active');
  document.documentElement.style.setProperty('--terminal-height', '0px');
}

/**
 * Spawns a new pseudo-terminal process and attaches an xterm.js instance.
 *
 * Resolves working directory contextually:
 * - If inside a project session: uses project root (`sidebarState.getActiveProjectPath()`).
 * - If inside a general chat session: uses default workspace (`~/.operon/workspace/`).
 */
export async function createNewTerminalTab(explicitWorkdir?: string): Promise<void> {
  if (!window.Terminal || !window.FitAddon) {
    console.error('[Terminal] xterm.js or FitAddon is not loaded.');
    return;
  }

  if (!terminalContainerEl) return;

  // 1. Resolve workdir based on active session type
  let resolvedWorkdir = explicitWorkdir;
  if (!resolvedWorkdir) {
    const activeProject = sidebarState.getActiveProjectPath();
    if (activeProject && activeProject.trim().length > 0) {
      resolvedWorkdir = activeProject.trim();
    } else {
      try {
        resolvedWorkdir = await getTerminalDefaultWorkdirIpc();
      } catch (err) {
        console.warn('[Terminal] Failed to get default workspace from backend:', err);
      }
    }
  }

  const tabId = `term_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`;
  const tabName = terminalState.getNextTabName();

  // 2. Create DOM wrapper element for xterm canvas (visible so fit addon can calculate true layout)
  const wrapperEl = document.createElement('div');
  wrapperEl.className = 'terminal-tab-wrapper';
  wrapperEl.id = `wrapper-${tabId}`;
  wrapperEl.style.display = 'block';
  terminalContainerEl.appendChild(wrapperEl);

  // 3. Instantiate xterm.js with Kode Mono font and Operon dark palette
  const term = new window.Terminal({
    fontFamily: "var(--mono-font-family, 'Kode Mono', monospace)",
    fontSize: 13,
    lineHeight: 1.25,
    cursorBlink: true,
    cursorStyle: 'block',
    theme: {
      background: '#121212',
      foreground: '#e0e0e0',
      cursor: '#ffffff',
      selectionBackground: 'rgba(255, 255, 255, 0.2)',
      black: '#1e1e1e',
      red: '#f14c4c',
      green: '#23d18b',
      yellow: '#f5f543',
      blue: '#3b8eea',
      magenta: '#d670d6',
      cyan: '#29b8db',
      white: '#e5e5e5',
      brightBlack: '#666666',
      brightRed: '#f14c4c',
      brightGreen: '#23d18b',
      brightYellow: '#f5f543',
      brightBlue: '#3b8eea',
      brightMagenta: '#d670d6',
      brightCyan: '#29b8db',
      brightWhite: '#e5e5e5',
    },
  });

  // 4. Load FitAddon
  const fitAddon = new window.FitAddon.FitAddon();
  term.loadAddon(fitAddon);
  term.open(wrapperEl);

  // 5. Compute full true dimensions immediately using FitAddon
  fitAddon.fit();
  const proposed = fitAddon.proposeDimensions();
  const initialCols = proposed && proposed.cols > 10 ? proposed.cols : 80;
  const initialRows = proposed && proposed.rows > 4 ? proposed.rows : 24;

  // 6. Create Tab Header Element
  const tabEl = document.createElement('div');
  tabEl.className = 'terminal-tab';
  tabEl.id = `tab-${tabId}`;
  tabEl.setAttribute('data-tab-id', tabId);

  const label = document.createElement('span');
  label.className = 'terminal-tab__name';
  label.textContent = tabName;

  const closeBtn = document.createElement('span');
  closeBtn.className = 'terminal-tab__close';
  closeBtn.setAttribute('role', 'button');
  closeBtn.setAttribute('title', 'Close tab');
  closeBtn.innerHTML = '<span class="terminal-tab__close-icon"></span>';

  tabEl.appendChild(label);
  tabEl.appendChild(closeBtn);

  // 7. Connect xterm I/O handlers to backend IPC
  term.onData((data: string) => {
    writeTerminalIpc(tabId, data).catch((err: unknown) => {
      console.error('[Terminal] Write error:', err);
    });
  });

  term.onResize(({ cols, rows }: { cols: number; rows: number }) => {
    resizeTerminalIpc(tabId, cols, rows).catch((err: unknown) => {
      console.error('[Terminal] Resize error:', err);
    });
  });

  // Tab click listener to switch active view
  tabEl.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).closest('.terminal-tab__close')) return;
    selectTerminalTab(tabId);
  });

  // Tab close click listener
  closeBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    closeTerminalTab(tabId);
  });

  const tabObj: TerminalTab = {
    id: tabId,
    name: tabName,
    term,
    fitAddon,
    wrapperEl,
    tabEl,
    workdir: resolvedWorkdir,
  };

  // 8. Spawn backend PTY process with true dimensions
  try {
    await createTerminalIpc(tabId, initialCols, initialRows, resolvedWorkdir);
    terminalState.addTab(tabObj);
    selectTerminalTab(tabId);
  } catch (err) {
    console.error(`[Terminal] Failed to spawn PTY session '${tabId}':`, err);
    wrapperEl.remove();
    tabEl.remove();
    term.dispose();
  }
}

/**
 * Switches the visible viewport to the specified terminal tab.
 */
export function selectTerminalTab(tabId: string): void {
  terminalState.setActiveTabId(tabId);

  const tabs = terminalState.getTabs();
  tabs.forEach((tab) => {
    if (tab.id === tabId) {
      tab.tabEl.classList.add('active');
      tab.wrapperEl.style.display = 'block';

      requestAnimationFrame(() => {
        try {
          tab.fitAddon.fit();
          tab.term.focus();
          if (tab.term.cols > 0 && tab.term.rows > 0) {
            resizeTerminalIpc(tabId, tab.term.cols, tab.term.rows).catch(() => {});
          }
        } catch (e) {
          console.debug('[Terminal] Fit error:', e);
        }
      });
    } else {
      tab.tabEl.classList.remove('active');
      tab.wrapperEl.style.display = 'none';
    }
  });
}

/**
 * Terminates and closes a terminal tab process and cleans up its DOM resources.
 */
export async function closeTerminalTab(tabId: string): Promise<void> {
  try {
    await closeTerminalIpc(tabId);
  } catch (err) {
    console.warn('[Terminal] Close IPC warning:', err);
  }
  removeTabResources(tabId);
}

/**
 * Handles when a PTY process exits naturally from the backend (e.g. typing `exit`).
 */
function handleTerminalProcessExited(tabId: string): void {
  removeTabResources(tabId);
}

function removeTabResources(tabId: string): void {
  const removed = terminalState.removeTab(tabId);
  if (removed) {
    try {
      removed.term.dispose();
    } catch {}
    removed.wrapperEl.remove();
    removed.tabEl.remove();
  }

  const activeId = terminalState.getActiveTabId();
  if (activeId) {
    selectTerminalTab(activeId);
  } else {
    closeTerminalPanel();
  }
}

/**
 * Renders tabs into `#terminal-tabs-container`.
 */
function renderTabs(): void {
  if (!tabsContainerEl) return;
  tabsContainerEl.innerHTML = '';

  const tabs = terminalState.getTabs();
  tabs.forEach((tab) => {
    tabsContainerEl?.appendChild(tab.tabEl);
  });
}

/**
 * Fits the character grid dimensions of the active tab to its bounding container.
 */
export function fitActiveTerminal(): void {
  const activeTab = terminalState.getActiveTab();
  if (activeTab && activeTab.wrapperEl.style.display !== 'none') {
    try {
      activeTab.fitAddon.fit();
      resizeTerminalIpc(activeTab.id, activeTab.term.cols, activeTab.term.rows).catch(() => {});
    } catch (e) {
      console.debug('[Terminal] Error fitting xterm:', e);
    }
  }
}

/**
 * Focuses keyboard input on the active terminal instance.
 */
export function focusActiveTerminal(): void {
  const activeTab = terminalState.getActiveTab();
  if (activeTab) {
    activeTab.term.focus();
  }
}

/**
 * Sets up vertical dragging on the resize handle.
 */
function setupDragResize(handle: HTMLElement): void {
  handle.addEventListener('mousedown', (e: MouseEvent) => {
    e.preventDefault();
    isResizing = true;
    panelEl?.classList.add('resizing');
    document.body.classList.add('terminal-resizing');

    const onMouseMove = (moveEvent: MouseEvent) => {
      if (!isResizing) return;

      let newHeight = window.innerHeight - moveEvent.clientY;
      const minHeight = 120;
      const maxHeight = window.innerHeight * 0.8;

      if (newHeight < minHeight) newHeight = minHeight;
      if (newHeight > maxHeight) newHeight = maxHeight;

      terminalState.setSavedHeight(newHeight);
      document.documentElement.style.setProperty('--terminal-height', `${newHeight}px`);
      fitActiveTerminal();
    };

    const onMouseUp = () => {
      isResizing = false;
      panelEl?.classList.remove('resizing');
      document.body.classList.remove('terminal-resizing');
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  });
}
