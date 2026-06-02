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

// Import the titlebar controller
import TitlebarController from './js/titlebar/titlebar.js';

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
