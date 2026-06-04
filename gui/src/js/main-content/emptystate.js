/**
 * Empty State Component
 * 
 * This module handles the empty state display in the main content area.
 * The empty state is shown when no messages are present (new session).
 * 
 * Functionality includes:
 * - Showing empty state on initialization
 * - Hiding empty state when first message is added
 * - Managing visibility of related elements (session title, messages)
 */

'use strict';

/**
 * EmptyStateController class
 * 
 * Manages empty state display and transitions.
 * Coordinates visibility of empty state, session title, and messages area.
 */
class EmptyStateController {
    /**
     * Constructor - Initializes the empty state controller
     * 
     * Sets up:
     * - References to empty state and related elements
     * - Initial visibility state
     */
    constructor() {
        // DOM element references
        this.emptyStateElement = null;
        this.sessionTitleElement = null;
        this.messagesElement = null;
        
        // State tracking
        this.isEmptyState = true;
        
        // Initialize
        this.init();
    }

    /**
     * Initialize the empty state controller
     * 
     * Sets up element references and initial visibility.
     */
    init() {
        try {
            this.emptyStateElement = document.getElementById('main-empty-state');
            this.sessionTitleElement = document.querySelector('.main-content__session-title');
            this.messagesElement = document.querySelector('.main-content__messages');
            
            if (!this.emptyStateElement || !this.sessionTitleElement || !this.messagesElement) {
                throw new Error('Required empty state elements not found');
            }
            
            // Initialize visibility - show empty state by default
            this.showEmptyState();
            
            console.log('Empty state controller initialized successfully');
        } catch (error) {
            console.error('Failed to initialize empty state controller:', error);
        }
    }

    /* ========================================================================
       EMPTY STATE MANAGEMENT
       ======================================================================== */

    /**
     * Show the empty state
     * 
     * Displays the empty state and hides the session title and messages.
     * Call this when there are no messages in the session.
     */
    showEmptyState() {
        if (!this.emptyStateElement || !this.sessionTitleElement || !this.messagesElement) {
            return;
        }

        this.emptyStateElement.classList.remove('hidden');
        this.sessionTitleElement.classList.add('hidden');
        this.messagesElement.classList.add('hidden');
        
        this.isEmptyState = true;

        console.log('Empty state shown');
    }

    /**
     * Hide the empty state
     * 
     * Hides the empty state and shows the session title and messages.
     * Call this when the first message is added to the session.
     */
    hideEmptyState() {
        if (!this.emptyStateElement || !this.sessionTitleElement || !this.messagesElement) {
            return;
        }

        this.emptyStateElement.classList.add('hidden');
        this.sessionTitleElement.classList.remove('hidden');
        this.messagesElement.classList.remove('hidden');
        
        this.isEmptyState = false;

        console.log('Empty state hidden');
    }

    /**
     * Check if currently in empty state
     * 
     * @returns {boolean} True if in empty state
     */
    getEmptyState() {
        return this.isEmptyState;
    }

    /**
     * Clear all messages and return to empty state
     * 
     * Useful when starting a new session or clearing chat history.
     */
    reset() {
        this.showEmptyState();
    }
}

/**
 * Initialize the empty state controller when DOM is ready
 * 
 * This creates a single instance of the EmptyStateController
 * and makes it globally accessible.
 */
let emptyStateController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        emptyStateController = new EmptyStateController();
        // Make globally accessible
        window.emptyStateController = emptyStateController;
    });
} else {
    // DOM is already loaded
    emptyStateController = new EmptyStateController();
    window.emptyStateController = emptyStateController;
}

// Export for potential use in other modules
export default EmptyStateController;
