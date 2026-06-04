/**
 * Session Title Component
 * 
 * This module handles the session title functionality including:
 * - Setting and updating the session title
 * - Managing title visibility
 * - Providing a simple API for title management
 * 
 * The session title appears at the top left of the main content area.
 */

'use strict';

/**
 * SessionTitleController class
 * 
 * Manages session title display and updates.
 * This class provides a simple interface for setting and getting the title.
 */
class SessionTitleController {
    /**
     * Constructor - Initializes the session title controller
     * 
     * Sets up:
     * - Reference to title element
     * - Default title
     */
    constructor() {
        // DOM element reference
        this.titleElement = null;
        
        // Default title
        this.defaultTitle = 'New Chat';
        
        // Initialize
        this.init();
    }

    /**
     * Initialize the session title controller
     * 
     * Sets up the title element reference.
     */
    init() {
        try {
            this.titleElement = document.getElementById('session-title');
            
            if (!this.titleElement) {
                throw new Error('Session title element not found');
            }
            
            console.log('Session title controller initialized successfully');
        } catch (error) {
            console.error('Failed to initialize session title controller:', error);
        }
    }

    /* ========================================================================
       TITLE MANAGEMENT
       ======================================================================== */

    /**
     * Set the session title
     * 
     * @param {string} title - The new title text
     */
    setTitle(title) {
        if (!this.titleElement) return;

        const newTitle = title && title.trim() ? title.trim() : this.defaultTitle;
        this.titleElement.textContent = newTitle;

        console.log('Session title updated:', newTitle);
    }

    /**
     * Get the current session title
     * 
     * @returns {string} The current title text
     */
    getTitle() {
        if (!this.titleElement) return this.defaultTitle;
        return this.titleElement.textContent || this.defaultTitle;
    }

    /**
     * Reset the title to default
     */
    resetTitle() {
        this.setTitle(this.defaultTitle);
    }

    /**
     * Update title based on first user message
     * 
     * This is a convenience method to automatically set the title
     * based on the first message in a conversation.
     * 
     * @param {string} message - The user's first message
     * @param {number} maxLength - Maximum length for the title (default 50)
     */
    setTitleFromMessage(message, maxLength = 50) {
        if (!message || !message.trim()) return;

        // Take first line or first few words
        let title = message.trim().split('\n')[0];

        // Truncate if too long
        if (title.length > maxLength) {
            title = title.substring(0, maxLength).trim() + '...';
        }

        this.setTitle(title);
    }

    /**
     * Show the title element
     */
    show() {
        if (!this.titleElement) return;
        const container = this.titleElement.closest('.main-content__session-title');
        if (container) {
            container.style.display = 'block';
        }
    }

    /**
     * Hide the title element
     */
    hide() {
        if (!this.titleElement) return;
        const container = this.titleElement.closest('.main-content__session-title');
        if (container) {
            container.style.display = 'none';
        }
    }
}

/**
 * Initialize the session title controller when DOM is ready
 * 
 * This creates a single instance of the SessionTitleController
 * and makes it globally accessible.
 */
let sessionTitleController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        sessionTitleController = new SessionTitleController();
        // Make globally accessible
        window.sessionTitleController = sessionTitleController;
    });
} else {
    // DOM is already loaded
    sessionTitleController = new SessionTitleController();
    window.sessionTitleController = sessionTitleController;
}

// Export for potential use in other modules
export default SessionTitleController;
