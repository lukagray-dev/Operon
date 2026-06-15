/**
 * Terminal Panel Component
 * 
 * Hello friend! This module implements the frontend controller for our bottom-docked
 * PowerShell terminal panel. It manages multiple tabs, PTY processes, resizing,
 * and xterm.js terminal emulator rendering.
 */

'use strict';

import * as IPC from '../shared/ipc.js';
import { showError } from '../shared/toast.js';

class TerminalController {
    constructor() {
        // Active terminal tabs state
        this.tabs = [];
        this.activeTabId = null;
        this.defaultWorkspace = null;
        this.nextTabNum = 1;
        
        // Resizing state
        this.isResizing = false;
        this.savedHeight = parseInt(localStorage.getItem('operon-terminal-height')) || 300;
        
        // DOM element references
        this.panelEl = null;
        this.toggleBtnEl = null;
        this.tabsContainerEl = null;
        this.addTabBtnEl = null;
        this.terminalContainerEl = null;
        this.resizeHandleEl = null;
        
        // Event listener unbinds
        this.unlistenOutput = null;
        this.unlistenClosed = null;
        
        // Auto-initialize
        this.init();
    }

    /**
     * Initialize DOM elements and register events
     */
    async init() {
        try {
            // Retrieve UI elements
            this.panelEl = document.getElementById('terminal-panel');
            this.toggleBtnEl = document.getElementById('terminal-toggle-btn');
            this.tabsContainerEl = document.getElementById('terminal-tabs-container');
            this.addTabBtnEl = document.getElementById('terminal-add-tab-btn');
            this.terminalContainerEl = document.getElementById('terminal-container');
            this.resizeHandleEl = document.getElementById('terminal-resize-handle');
            
            if (!this.panelEl || !this.toggleBtnEl) {
                console.warn('Terminal DOM elements not found. Skipping initialization.');
                return;
            }

            // Load default workspace path from backend configuration
            if (IPC.isTauriAvailable()) {
                this.defaultWorkspace = await IPC.getDefaultWorkspace();
            }

            // Register toggle click
            this.toggleBtnEl.addEventListener('click', () => this.togglePanel());
            
            // Register panel button controls
            if (this.addTabBtnEl) {
                this.addTabBtnEl.addEventListener('click', () => this.createNewTab());
            }
            
            // Register resizing drag listeners
            if (this.resizeHandleEl) {
                this.resizeHandleEl.addEventListener('mousedown', (e) => this.startResize(e));
            }
            
            // Register layout auto-resize on window resize
            window.addEventListener('resize', () => this.fitActiveTerminal());
            
            // Listen to backend IPC events
            this.setupTauriEventListeners();
            
            console.log('Terminal controller initialized successfully');
        } catch (error) {
            console.error('Failed to initialize terminal controller:', error);
        }
    }

    /**
     * Set up Tauri event listeners for streaming terminal outputs
     */
    async setupTauriEventListeners() {
        if (!IPC.isTauriAvailable()) return;
        
        try {
            const { listen } = window.__TAURI__.event;
            
            // Listen to terminal output stream
            this.unlistenOutput = await listen('terminal-output', (event) => {
                const { id, data } = event.payload;
                const tab = this.tabs.find(t => t.id === id);
                if (tab && tab.term) {
                    tab.term.write(data);
                }
            });
            
            // Listen to terminal exit
            this.unlistenClosed = await listen('terminal-closed', (event) => {
                const { id } = event.payload;
                this.handleTerminalExited(id);
            });
        } catch (error) {
            console.error('Failed to set up Tauri event listeners for terminal:', error);
        }
    }

    /**
     * Start the vertical dragging action to resize the terminal height
     */
    startResize(e) {
        e.preventDefault();
        this.isResizing = true;
        this.panelEl.classList.add('resizing');
        document.body.classList.add('terminal-resizing');
        
        const onMouseMove = (moveEvent) => {
            if (!this.isResizing) return;
            
            // Calculate terminal height from the bottom of the viewport
            let newHeight = window.innerHeight - moveEvent.clientY;
            
            // Apply minimum and maximum constraints
            const minHeight = 120;
            const maxHeight = window.innerHeight * 0.8;
            
            if (newHeight < minHeight) newHeight = minHeight;
            if (newHeight > maxHeight) newHeight = maxHeight;
            
            this.savedHeight = newHeight;
            localStorage.setItem('operon-terminal-height', newHeight);
            
            // Set variables and adjust xterm fitting
            document.documentElement.style.setProperty('--terminal-height', `${newHeight}px`);
            this.fitActiveTerminal();
        };
        
        const onMouseUp = () => {
            this.isResizing = false;
            if (this.panelEl) {
                this.panelEl.classList.remove('resizing');
            }
            document.body.classList.remove('terminal-resizing');
            window.removeEventListener('mousemove', onMouseMove);
            window.removeEventListener('mouseup', onMouseUp);
        };
        
        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
    }

    /**
     * Toggle the visibility of the terminal panel
     */
    async togglePanel() {
        const isCollapsed = this.panelEl.classList.contains('collapsed');
        
        if (isCollapsed) {
            // Show the terminal panel
            this.panelEl.classList.remove('collapsed');
            this.toggleBtnEl.classList.add('active');
            
            // Apply height
            document.documentElement.style.setProperty('--terminal-height', `${this.savedHeight}px`);
            
            // If no active tabs exist, launch a default PowerShell session
            if (this.tabs.length === 0) {
                await this.createNewTab();
            } else {
                this.fitActiveTerminal();
                this.focusActiveTerminal();
            }
        } else {
            // Collapse panel
            this.closePanel();
        }
    }

    /**
     * Collapse the panel and clear layout state
     */
    closePanel() {
        if (this.panelEl) {
            this.panelEl.classList.add('collapsed');
        }
        if (this.toggleBtnEl) {
            this.toggleBtnEl.classList.remove('active');
        }
        document.documentElement.style.setProperty('--terminal-height', '0px');
    }

    /**
     * Create a new terminal tab process and mount its xterm.js client
     */
    async createNewTab() {
        if (typeof window.Terminal === 'undefined') {
            showError('xterm.js library is not loaded.');
            return;
        }

        // Generate a unique ID for this tab session
        const tabId = `term_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
        const tabName = `pwsh ${this.nextTabNum++}`;

        // Determine correct start directory
        let workdir = null;
        if (window.sessionManager && window.sessionManager.currentProjectDir) {
            workdir = window.sessionManager.currentProjectDir;
        } else {
            workdir = this.defaultWorkspace;
        }

        // 1. Create a wrapper element for xterm inside terminalContainer
        const wrapper = document.createElement('div');
        wrapper.className = 'terminal-tab-wrapper';
        wrapper.id = `wrapper-${tabId}`;
        wrapper.style.width = '100%';
        wrapper.style.height = '100%';
        wrapper.style.display = 'none'; // Hidden by default until activated
        this.terminalContainerEl.appendChild(wrapper);

        // 2. Initialize xterm.js instance
        const term = new window.Terminal({
            fontFamily: 'var(--font-family-mono), monospace',
            fontSize: 12,
            lineHeight: 1.2,
            theme: {
                background: '#0c0c0c',
                foreground: '#e0e0e0',
                cursor: '#ffffff',
                selectionBackground: 'rgba(255, 255, 255, 0.15)',
                black: '#1e1e1d',
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
                brightWhite: '#e5e5e5'
            },
            cursorBlink: true,
            cursorStyle: 'block'
        });

        // 3. Load the FitAddon to handle responsive container sizing
        const fitAddon = new window.FitAddon.FitAddon();
        term.loadAddon(fitAddon);
        
        // Open/render it inside the wrapper element
        term.open(wrapper);

        // 4. Create the Tab header element
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
        closeBtn.innerHTML = `<img src="./assets/icons/action/close.svg" alt="Close" class="terminal-tab__close-icon" />`;
        
        tabEl.appendChild(label);
        tabEl.appendChild(closeBtn);
        this.tabsContainerEl.appendChild(tabEl);

        // Define our client tab object
        const tabObj = {
            id: tabId,
            name: tabName,
            term: term,
            fitAddon: fitAddon,
            wrapperEl: wrapper,
            tabEl: tabEl
        };

        this.tabs.push(tabObj);

        // 5. Connect handlers for reading/writing inputs
        term.onData(data => {
            IPC.writeTerminal(tabId, data).catch(err => console.error(err));
        });

        // Trigger resize events back to backend PTY when grid changes
        term.onResize(({ cols, rows }) => {
            IPC.resizeTerminal(tabId, cols, rows).catch(err => console.error(err));
        });

        // Register tab click listener to switch view
        tabEl.addEventListener('click', (e) => {
            if (e.target.closest('.terminal-tab__close')) return;
            this.selectTab(tabId);
        });

        // Register tab close click
        closeBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            this.closeTab(tabId);
        });

        // 6. Spawn the backend PTY command
        try {
            // Start PTY with a default 80x24 size, immediately resized below via fitting
            await IPC.createTerminal(tabId, 80, 24, workdir);
            
            // Activate and fit the new tab
            this.selectTab(tabId);
        } catch (error) {
            showError(`Failed to open terminal process: ${error}`);
            this.removeTabFromDOM(tabId);
        }
    }

    /**
     * Switch view to the specified active terminal tab ID
     */
    selectTab(tabId) {
        this.activeTabId = tabId;

        this.tabs.forEach(tab => {
            if (tab.id === tabId) {
                tab.tabEl.classList.add('active');
                tab.wrapperEl.style.display = 'block';
                
                // Trigger fit and focus on next tick to ensure visibility renders correctly
                setTimeout(() => {
                    tab.fitAddon.fit();
                    tab.term.focus();
                    
                    // Trigger manual resize call to align PTY rows/cols
                    const cols = tab.term.cols;
                    const rows = tab.term.rows;
                    IPC.resizeTerminal(tabId, cols, rows).catch(err => console.error(err));
                }, 10);
            } else {
                tab.tabEl.classList.remove('active');
                tab.wrapperEl.style.display = 'none';
            }
        });
    }

    /**
     * Terminate the backend process and close the frontend tab
     */
    async closeTab(tabId) {
        try {
            await IPC.closeTerminal(tabId);
        } catch (error) {
            console.error('Failed to close PTY process:', error);
        }
        this.removeTabFromDOM(tabId);
    }

    /**
     * Backend signaled that the PTY process has exited
     */
    handleTerminalExited(tabId) {
        console.log(`Backend process for terminal tab '${tabId}' exited.`);
        this.removeTabFromDOM(tabId);
    }

    /**
     * Remove DOM elements and local states for a terminal tab
     */
    removeTabFromDOM(tabId) {
        const index = this.tabs.findIndex(t => t.id === tabId);
        if (index === -1) return;

        const tab = this.tabs[index];
        
        // Clean up xterm instance resources
        try {
            tab.term.dispose();
        } catch (e) {
            console.error(e);
        }
        
        // Remove elements from DOM
        if (tab.wrapperEl) tab.wrapperEl.remove();
        if (tab.tabEl) tab.tabEl.remove();
        
        // Remove from array
        this.tabs.splice(index, 1);
        
        // Handle focus redirection
        if (this.activeTabId === tabId) {
            if (this.tabs.length > 0) {
                // Select last tab
                const nextTab = this.tabs[this.tabs.length - 1];
                this.selectTab(nextTab.id);
            } else {
                this.activeTabId = null;
                this.closePanel();
            }
        }
    }

    /**
     * Fit the character dimensions of the active tab to its body container
     */
    fitActiveTerminal() {
        if (!this.activeTabId) return;
        const activeTab = this.tabs.find(t => t.id === this.activeTabId);
        if (activeTab && activeTab.wrapperEl.style.display !== 'none') {
            try {
                activeTab.fitAddon.fit();
                
                // Align backend PTY with the new cols/rows
                const cols = activeTab.term.cols;
                const rows = activeTab.term.rows;
                IPC.resizeTerminal(this.activeTabId, cols, rows).catch(err => console.error(err));
            } catch (e) {
                console.error('Error fitting xterm:', e);
            }
        }
    }

    /**
     * Focus the keystroke input on the active terminal instance
     */
    focusActiveTerminal() {
        if (!this.activeTabId) return;
        const activeTab = this.tabs.find(t => t.id === this.activeTabId);
        if (activeTab) {
            activeTab.term.focus();
        }
    }
}

// Auto-initialize once DOM is fully loaded
let terminalController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        terminalController = new TerminalController();
        window.terminalController = terminalController;
    });
} else {
    terminalController = new TerminalController();
    window.terminalController = terminalController;
}

export default TerminalController;
