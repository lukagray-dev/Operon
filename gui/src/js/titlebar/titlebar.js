/**
 * Titlebar Component
 * 
 * This module handles all functionality for the custom titlebar including:
 * - Window control operations (minimize, maximize, close)
 * - Menu interactions (dropdown open/close)
 * - View operations (zoom, reload)
 * - External link opening
 * - Navigation controls (back/forward - UI only for now)
 * 
 * The titlebar integrates with Tauri's window APIs for native window management.
 */

'use strict';

import { URLS } from '../shared/tokens.js';

/**
 * TitlebarController class
 * 
 * Manages all titlebar interactions and window operations.
 * This class follows the single responsibility principle by separating
 * concerns into focused private methods.
 */
class TitlebarController {
    /**
     * Constructor - Initializes the titlebar controller
     * 
     * Sets up:
     * - Current zoom level tracking
     * - Active menu state
     * - Window maximized state tracking
     * - Event listeners for all interactive elements
     */
    constructor() {
        // Current zoom level (1.0 = 100%)
        this.currentZoom = 1.0;
        
        // Track which menu dropdown is currently open
        this.activeMenuId = null;
        
        // Track window maximized state
        this.isMaximized = false;
        
        // Initialize the titlebar
        this.init();
    }

    /**
     * Initialize all titlebar functionality
     * 
     * This is the main entry point that sets up all event listeners
     * and initializes the window state.
     */
    async init() {
        try {
            // Initialize window state tracking
            await this.initWindowState();
            
            // Set up all event listeners
            this.initEventListeners();
            
            console.log('Titlebar initialized successfully');
        } catch (error) {
            console.error('Failed to initialize titlebar:', error);
        }
    }

    /**
     * Initialize window state tracking
     * 
     * Checks if window is currently maximized and updates UI accordingly.
     * Uses Tauri's window API to get the current window state.
     */
    async initWindowState() {
        try {
            // Check if Tauri API is available
            if (window.__TAURI__) {
                const { getCurrentWindow } = window.__TAURI__.window;
                const appWindow = getCurrentWindow();
                this.isMaximized = await appWindow.isMaximized();
                
                // Update body class based on maximized state
                this.updateMaximizedState(this.isMaximized);
                
                // Listen for window resize events to track maximize state
                await appWindow.listen('tauri://resize', async () => {
                    this.isMaximized = await appWindow.isMaximized();
                    this.updateMaximizedState(this.isMaximized);
                });
            }
        } catch (error) {
            console.error('Failed to initialize window state:', error);
        }
    }

    /**
     * Update UI based on window maximized state
     * 
     * @param {boolean} isMaximized - Whether window is maximized
     */
    updateMaximizedState(isMaximized) {
        if (isMaximized) {
            document.body.classList.add('is-maximized');
        } else {
            document.body.classList.remove('is-maximized');
        }
    }

    /**
     * Initialize all event listeners
     * 
     * Sets up listeners for:
     * - Navigation buttons (back/forward)
     * - Menu items (Files, View, Window, Help)
     * - Window control buttons (minimize, maximize, close)
     * - Outside clicks to close dropdowns
     */
    initEventListeners() {
        // Navigation buttons
        this.setupNavigationListeners();
        
        // Menu items and dropdowns
        this.setupMenuListeners();
        
        // Window control buttons
        this.setupWindowControlListeners();
        
        // Close dropdowns when clicking outside
        this.setupOutsideClickListener();
    }

    /**
     * Setup navigation button listeners (back/forward)
     * 
     * Currently these are UI-only and don't control anything.
     * Future implementation will add navigation history management.
     */
    setupNavigationListeners() {
        const backBtn = document.getElementById('titlebar-nav-back');
        const forwardBtn = document.getElementById('titlebar-nav-forward');

        if (backBtn) {
            backBtn.addEventListener('click', () => {
                console.log('Back button clicked - navigation not implemented yet');
                // TODO: Implement navigation history
            });
        }

        if (forwardBtn) {
            forwardBtn.addEventListener('click', () => {
                console.log('Forward button clicked - navigation not implemented yet');
                // TODO: Implement navigation history
            });
        }
    }

    /**
     * Setup menu item listeners and dropdown handlers
     * 
     * Handles opening/closing menu dropdowns and routing actions
     * to appropriate handlers based on the menu item clicked.
     */
    setupMenuListeners() {
        // Get all menu items that have dropdowns
        const menuItems = document.querySelectorAll('.titlebar__menu-item');

        menuItems.forEach(menuItem => {
            menuItem.addEventListener('click', (e) => {
                e.stopPropagation();
                
                const menuId = menuItem.getAttribute('data-menu');
                
                // Toggle dropdown
                if (this.activeMenuId === menuId) {
                    // Close if already open
                    this.closeAllMenus();
                } else {
                    // Open this menu, close others
                    this.closeAllMenus();
                    this.openMenu(menuId);
                }
            });
        });

        // Setup individual menu item actions
        this.setupFilesMenuActions();
        this.setupViewMenuActions();
        this.setupWindowMenuActions();
        this.setupHelpMenuActions();
    }

    /**
     * Setup Files menu action handlers
     * 
     * Menu items:
     * - New conversation (not implemented yet)
     * - Settings (not implemented yet)
     * - Open project (not implemented yet)
     */
    setupFilesMenuActions() {
        const newConversation = document.getElementById('menu-new-conversation');
        const settings = document.getElementById('menu-settings');
        const openProject = document.getElementById('menu-open-project');

        if (newConversation) {
            newConversation.addEventListener('click', () => {
                console.log('New conversation - not implemented yet');
                this.closeAllMenus();
                // TODO: Implement new conversation
            });
        }

        if (settings) {
            settings.addEventListener('click', () => {
                console.log('Settings - not implemented yet');
                this.closeAllMenus();
                // TODO: Open settings dialog
            });
        }

        if (openProject) {
            openProject.addEventListener('click', () => {
                console.log('Open project - not implemented yet');
                this.closeAllMenus();
                // TODO: Open file picker for project
            });
        }
    }

    /**
     * Setup View menu action handlers
     * 
     * Menu items:
     * - Reload - reloads the application
     * - Zoom in - increases zoom level
     * - Zoom out - decreases zoom level
     * - Actual size - resets zoom to 100%
     */
    setupViewMenuActions() {
        const reload = document.getElementById('menu-reload');
        const zoomIn = document.getElementById('menu-zoom-in');
        const zoomOut = document.getElementById('menu-zoom-out');
        const actualSize = document.getElementById('menu-actual-size');

        if (reload) {
            reload.addEventListener('click', () => {
                this.reloadWindow();
                this.closeAllMenus();
            });
        }

        if (zoomIn) {
            zoomIn.addEventListener('click', () => {
                this.zoomIn();
                this.closeAllMenus();
            });
        }

        if (zoomOut) {
            zoomOut.addEventListener('click', () => {
                this.zoomOut();
                this.closeAllMenus();
            });
        }

        if (actualSize) {
            actualSize.addEventListener('click', () => {
                this.resetZoom();
                this.closeAllMenus();
            });
        }
    }

    /**
     * Setup Window menu action handlers
     * 
     * Menu items:
     * - Close window - closes the window (same as close button)
     * - Exit - exits the application
     */
    setupWindowMenuActions() {
        const closeWindow = document.getElementById('menu-close-window');
        const exit = document.getElementById('menu-exit');

        if (closeWindow) {
            closeWindow.addEventListener('click', () => {
                this.closeWindow();
                this.closeAllMenus();
            });
        }

        if (exit) {
            exit.addEventListener('click', () => {
                this.exitApp();
                this.closeAllMenus();
            });
        }
    }

    /**
     * Setup Help menu action handlers
     * 
     * Menu items:
     * - Documentation - opens GitHub docs in browser
     * - Check for update (not implemented yet)
     * - Report bug - opens GitHub issues in browser
     * - About (not implemented yet)
     * - Follow creator - opens Instagram in browser
     * - See repo - opens GitHub repository in browser
     */
    setupHelpMenuActions() {
        const documentation = document.getElementById('menu-documentation');
        const checkUpdate = document.getElementById('menu-check-update');
        const reportBug = document.getElementById('menu-report-bug');
        const about = document.getElementById('menu-about');
        const followCreator = document.getElementById('menu-follow-creator');
        const seeRepo = document.getElementById('menu-see-repo');

        if (documentation) {
            documentation.addEventListener('click', () => {
                this.openUrl(URLS.DOCUMENTATION);
                this.closeAllMenus();
            });
        }

        if (checkUpdate) {
            checkUpdate.addEventListener('click', () => {
                console.log('Check for update - not implemented yet');
                this.closeAllMenus();
                // TODO: Implement update checker
            });
        }

        if (reportBug) {
            reportBug.addEventListener('click', () => {
                this.openUrl(URLS.REPORT_BUG);
                this.closeAllMenus();
            });
        }

        if (about) {
            about.addEventListener('click', () => {
                console.log('About - not implemented yet');
                this.closeAllMenus();
                // TODO: Show about dialog
            });
        }

        if (followCreator) {
            followCreator.addEventListener('click', () => {
                this.openUrl(URLS.CREATOR_INSTAGRAM);
                this.closeAllMenus();
            });
        }

        if (seeRepo) {
            seeRepo.addEventListener('click', () => {
                this.openUrl(URLS.REPOSITORY);
                this.closeAllMenus();
            });
        }
    }

    /**
     * Setup window control button listeners
     * 
     * Handles minimize, maximize/unmaximize, and close operations
     * using Tauri's window API.
     */
    setupWindowControlListeners() {
        const minimizeBtn = document.getElementById('titlebar-minimize');
        const maximizeBtn = document.getElementById('titlebar-maximize');
        const closeBtn = document.getElementById('titlebar-close');

        if (minimizeBtn) {
            minimizeBtn.addEventListener('click', () => this.minimizeWindow());
        }

        if (maximizeBtn) {
            maximizeBtn.addEventListener('click', () => this.toggleMaximize());
        }

        if (closeBtn) {
            closeBtn.addEventListener('click', () => this.closeWindow());
        }
    }

    /**
     * Setup listener to close menus when clicking outside
     * 
     * This provides intuitive UX by closing dropdowns when user
     * clicks anywhere outside the menu area.
     */
    setupOutsideClickListener() {
        document.addEventListener('click', (e) => {
            // Check if click is outside menu items
            if (!e.target.closest('.titlebar__menu-item')) {
                this.closeAllMenus();
            }
        });
    }

    /**
     * Open a specific menu dropdown
     * 
     * @param {string} menuId - The ID of the menu to open
     */
    openMenu(menuId) {
        const menuItem = document.querySelector(`[data-menu="${menuId}"]`);
        if (menuItem) {
            menuItem.classList.add('active');
            this.activeMenuId = menuId;
        }
    }

    /**
     * Close all menu dropdowns
     * 
     * Removes active class from all menu items and resets active menu tracking.
     */
    closeAllMenus() {
        const activeMenus = document.querySelectorAll('.titlebar__menu-item.active');
        activeMenus.forEach(menu => menu.classList.remove('active'));
        this.activeMenuId = null;
    }

    /* ========================================================================
       WINDOW CONTROL OPERATIONS
       ======================================================================== */

    /**
     * Minimize the window
     * 
     * Uses Tauri's window API to minimize the application window.
     */
    async minimizeWindow() {
        try {
            if (window.__TAURI__) {
                const { getCurrentWindow } = window.__TAURI__.window;
                const appWindow = getCurrentWindow();
                await appWindow.minimize();
            } else {
                console.warn('Tauri API not available - minimize not supported');
            }
        } catch (error) {
            console.error('Failed to minimize window:', error);
        }
    }

    /**
     * Toggle window maximize/unmaximize
     * 
     * If window is maximized, restore to normal size.
     * If window is normal size, maximize it.
     */
    async toggleMaximize() {
        try {
            if (window.__TAURI__) {
                const { getCurrentWindow } = window.__TAURI__.window;
                const appWindow = getCurrentWindow();
                await appWindow.toggleMaximize();
            } else {
                console.warn('Tauri API not available - maximize not supported');
            }
        } catch (error) {
            console.error('Failed to toggle maximize:', error);
        }
    }

    /**
     * Close the window
     * 
     * Uses Tauri's window API to close the application window.
     * This is the same as clicking the close button.
     */
    async closeWindow() {
        try {
            if (window.__TAURI__) {
                const { getCurrentWindow } = window.__TAURI__.window;
                const appWindow = getCurrentWindow();
                await appWindow.close();
            } else {
                console.warn('Tauri API not available - close not supported');
            }
        } catch (error) {
            console.error('Failed to close window:', error);
        }
    }

    /**
     * Exit the application
     * 
     * Uses Tauri's process API to exit the entire application.
     * This is different from just closing the window.
     */
    async exitApp() {
        try {
            if (window.__TAURI__ && window.__TAURI__.process) {
                await window.__TAURI__.process.exit(0);
            } else {
                // Fallback to closing window
                await this.closeWindow();
            }
        } catch (error) {
            console.error('Failed to exit app:', error);
        }
    }

    /* ========================================================================
       VIEW OPERATIONS
       ======================================================================== */

    /**
     * Reload the window
     * 
     * Uses Tauri's webview API to reload the current page.
     * Equivalent to pressing F5 or Ctrl+R.
     */
    async reloadWindow() {
        try {
            if (window.__TAURI__) {
                const { getCurrentWebview } = window.__TAURI__.webview;
                const webview = getCurrentWebview();
                // Use webview reload if available
                if (webview.reload) {
                    await webview.reload();
                } else {
                    // Fallback to location reload
                    window.location.reload();
                }
            } else {
                // Fallback to standard reload
                window.location.reload();
            }
        } catch (error) {
            console.error('Failed to reload window:', error);
            // Fallback to location reload
            window.location.reload();
        }
    }

    /**
     * Zoom in
     * 
     * Increases the zoom level by 10% (0.1).
     * Maximum zoom is 200% (2.0).
     */
    zoomIn() {
        const newZoom = Math.min(this.currentZoom + 0.1, 2.0);
        this.setZoom(newZoom);
    }

    /**
     * Zoom out
     * 
     * Decreases the zoom level by 10% (0.1).
     * Minimum zoom is 50% (0.5).
     */
    zoomOut() {
        const newZoom = Math.max(this.currentZoom - 0.1, 0.5);
        this.setZoom(newZoom);
    }

    /**
     * Reset zoom to actual size (100%)
     * 
     * Sets zoom level back to 1.0 (100%).
     */
    resetZoom() {
        this.setZoom(1.0);
    }

    /**
     * Set zoom level
     * 
     * Applies the zoom level to the document body using CSS transform.
     * This scales the entire application UI.
     * 
     * @param {number} zoomLevel - The zoom level (0.5 to 2.0)
     */
    async setZoom(zoomLevel) {
        try {
            this.currentZoom = zoomLevel;
            
            // Try to use Tauri's webview setZoom if available
            if (window.__TAURI__) {
                const { getCurrentWebview } = window.__TAURI__.webview;
                const webview = getCurrentWebview();
                if (webview.setZoom) {
                    await webview.setZoom(zoomLevel);
                    console.log(`Zoom set to ${Math.round(zoomLevel * 100)}%`);
                    return;
                }
            }
            
            // Fallback to CSS zoom (less ideal but works)
            document.body.style.zoom = `${zoomLevel}`;
            console.log(`Zoom set to ${Math.round(zoomLevel * 100)}% (CSS fallback)`);
        } catch (error) {
            console.error('Failed to set zoom:', error);
            // Fallback to CSS zoom
            document.body.style.zoom = `${zoomLevel}`;
        }
    }

    /* ========================================================================
       EXTERNAL OPERATIONS
       ======================================================================== */

    /**
     * Open a URL in the default browser
     * 
     * Uses Tauri's opener plugin to open URLs externally.
     * This is secure and follows Tauri's security best practices.
     * 
     * @param {string} url - The URL to open
     */
    async openUrl(url) {
        try {
            if (window.__TAURI__ && window.__TAURI__.opener) {
                // Use the opener plugin to open URLs
                await window.__TAURI__.opener.openUrl(url);
                console.log(`Opened URL: ${url}`);
            } else {
                // Fallback to window.open for development
                window.open(url, '_blank');
                console.warn('Tauri opener plugin not available - using window.open fallback');
            }
        } catch (error) {
            console.error(`Failed to open URL ${url}:`, error);
            // Fallback to window.open
            window.open(url, '_blank');
        }
    }
}

/**
 * Initialize the titlebar when DOM is ready
 * 
 * This creates a single instance of the TitlebarController
 * and makes it globally accessible for debugging.
 */
let titlebarController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        titlebarController = new TitlebarController();
    });
} else {
    // DOM is already loaded
    titlebarController = new TitlebarController();
}

// Export for potential use in other modules
export default TitlebarController;
