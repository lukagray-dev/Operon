/**
 * Operon GUI - Main JavaScript Entry Point
 * 
 * This file initializes the Operon GUI application using vanilla JavaScript
 * with strict ES6 syntax. No frameworks or build tools required.
 * 
 * This is the main entry point that coordinates all components including:
 * - Titlebar component
 * - Main content area
 * - Sidebars (left and right)
 * - Any other global functionality
 */

'use strict';

// Load titlebar component (initializes itself on load)
import './js/titlebar/titlebar.js';

// Load left sidebar component (initializes itself on load)
import './js/left-sidebar/left-sidebar.js';

// Load input panel component (initializes itself on load)
import './js/main-content/inputpanel.js';

// Load user message component (initializes itself on load)
import './js/main-content/usermessage.js';

// Load assistant message component (initializes itself on load)
import './js/main-content/assistantmessage.js';

// Load session title component (initializes itself on load)
import './js/main-content/sessiontitle.js';

// Load empty state component (initializes itself on load)
import './js/main-content/emptystate.js';

// Load session manager to handle agent events and chats history
import './js/shared/session-manager.js';

// Initialize the settings panel (builds the overlay DOM node once on startup)
import { initSettingsPanel } from './js/settings/settings-panel.js';

/**
 * Main application class
 * 
 * Coordinates all components and manages the application lifecycle.
 * Follows the single responsibility principle by delegating specific
 * functionality to dedicated controller classes.
 */
class OperonApp {
    /**
     * Constructor - Initialize the application
     * 
     * Sets up all controllers and initializes the application state.
     */
    constructor() {
        // Store reference to titlebar controller (already initialized by its own module)
        this.titlebarController = null;
        
        // Initialize the application
        this.init();
    }

    /**
     * Initialize the application
     * 
     * Sets up all components in the correct order and handles
     * any initialization errors gracefully.
     */
    async init() {
        try {
            // Build the settings dialog overlay and inject it into <body>.
            // Must run before the user can click the sidebar Settings button.
            initSettingsPanel();

            // Initialize event listeners for main content
            this.initializeEventListeners();
            
            // Adjust layout to account for titlebar
            this.adjustLayoutForTitlebar();
            
            console.log('Operon GUI initialized successfully');
        } catch (error) {
            console.error('Failed to initialize Operon GUI:', error);
        }
    }

    /**
     * Adjust layout to account for titlebar height
     * 
     * Ensures the main content area doesn't overlap with the titlebar
     * by adding top padding equal to the titlebar height.
     */
    adjustLayoutForTitlebar() {
        const app = document.getElementById('app');
        if (app) {
            // Add top padding to prevent content from being hidden behind titlebar
            app.style.paddingTop = 'var(--titlebar-height)';
        }
    }

    /**
     * Initialize all event listeners for the main application
     * 
     * This sets up listeners for the main content area.
     * Component-specific listeners are handled by their respective controllers.
     */
    initializeEventListeners() {
        // Main application event listeners will be added here
        // as we build out the functionality
    }
}

/**
 * Initialize the application when DOM is fully loaded
 * 
 * This ensures all DOM elements are available before we try to
 * access or manipulate them.
 */
let operonApp = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        operonApp = new OperonApp();
    });
} else {
    // DOM is already loaded
    operonApp = new OperonApp();
}

// Export for debugging and potential external use
export default OperonApp;
