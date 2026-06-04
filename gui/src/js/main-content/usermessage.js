/**
 * User Message Component
 * 
 * This module handles all functionality for user messages including:
 * - Rendering user messages in the chat area
 * - Truncating long messages (more than 10 lines)
 * - Show more/less toggle for long messages
 * - Edit action (opens message in input for editing)
 * - Copy action (copies message to clipboard)
 * - Dynamic message creation from input panel
 * 
 * User messages appear on the right side with titlebar-colored bubbles.
 */

'use strict';

/**
 * UserMessageController class
 * 
 * Manages user message rendering, truncation, and actions.
 * This class follows the single responsibility principle by separating
 * concerns into focused private methods.
 */
class UserMessageController {
    /**
     * Constructor - Initializes the user message controller
     * 
     * Sets up:
     * - Reference to messages container
     * - Line height and max lines for truncation
     * - Message ID counter
     */
    constructor() {
        // DOM element references
        this.messagesContainer = null;
        
        // Truncation settings
        this.lineHeight = 20; // Must match CSS --user-message-line-height
        this.maxLines = 10;
        this.maxHeight = this.lineHeight * this.maxLines;
        
        // Message ID counter for unique identification
        this.messageIdCounter = 0;
        
        // Initialize
        this.init();
    }

    /**
     * Initialize the user message controller
     * 
     * Sets up the messages container reference.
     */
    init() {
        try {
            this.messagesContainer = document.querySelector('.main-content__messages');
            
            if (!this.messagesContainer) {
                throw new Error('Messages container not found');
            }
            
            console.log('User message controller initialized successfully');
        } catch (error) {
            console.error('Failed to initialize user message controller:', error);
        }
    }

    /* ========================================================================
       MESSAGE RENDERING
       ======================================================================== */

    /**
     * Create and render a user message
     * 
     * @param {string} content - The message content
     * @returns {HTMLElement} The created message element
     */
    createMessage(content) {
        if (!content || !content.trim()) {
            console.warn('Cannot create empty message');
            return null;
        }

        // Generate unique message ID
        const messageId = `user-message-${this.messageIdCounter++}`;

        // Create message wrapper
        const messageDiv = document.createElement('div');
        messageDiv.className = 'user-message';
        messageDiv.setAttribute('data-message-id', messageId);

        // Create message bubble
        const bubbleDiv = document.createElement('div');
        bubbleDiv.className = 'user-message__bubble';

        // Create content wrapper
        const contentDiv = document.createElement('div');
        contentDiv.className = 'user-message__content';
        contentDiv.textContent = content;

        // Check if message needs truncation
        const needsTruncation = this.checkIfTruncationNeeded(content);

        if (needsTruncation) {
            contentDiv.classList.add('truncated');
        }

        bubbleDiv.appendChild(contentDiv);

        // Add show more button if needed
        if (needsTruncation) {
            const showMoreBtn = this.createShowMoreButton(messageId);
            bubbleDiv.appendChild(showMoreBtn);
        }

        messageDiv.appendChild(bubbleDiv);

        // Create actions row
        const actionsDiv = this.createActionsRow(messageId, content);
        messageDiv.appendChild(actionsDiv);

        return messageDiv;
    }

    /**
     * Check if message needs truncation
     * 
     * @param {string} content - The message content
     * @returns {boolean} Whether truncation is needed
     */
    checkIfTruncationNeeded(content) {
        // Count number of lines
        const lines = content.split('\n');
        return lines.length > this.maxLines;
    }

    /**
     * Create show more/less button
     * 
     * @param {string} messageId - The message ID
     * @returns {HTMLElement} The show more button
     */
    createShowMoreButton(messageId) {
        const button = document.createElement('button');
        button.className = 'user-message__show-more';
        button.textContent = 'Show more';
        button.setAttribute('data-message-id', messageId);
        button.setAttribute('data-state', 'collapsed');

        // Add click listener
        button.addEventListener('click', () => {
            this.toggleShowMore(messageId);
        });

        return button;
    }

    /**
     * Create actions row (Edit and Copy buttons)
     * 
     * @param {string} messageId - The message ID
     * @param {string} content - The message content
     * @returns {HTMLElement} The actions container
     */
    createActionsRow(messageId, content) {
        const actionsDiv = document.createElement('div');
        actionsDiv.className = 'user-message__actions';

        // Edit button
        const editBtn = document.createElement('button');
        editBtn.className = 'user-message__action-btn';
        editBtn.setAttribute('data-action', 'edit');
        editBtn.setAttribute('data-message-id', messageId);

        const editIcon = document.createElement('img');
        editIcon.src = './assets/icons/main-content/messages/user/edit.svg';
        editIcon.alt = 'Edit';
        editIcon.className = 'user-message__action-icon';

        editBtn.appendChild(editIcon);
        // Text removed - only SVG icon

        // Copy button
        const copyBtn = document.createElement('button');
        copyBtn.className = 'user-message__action-btn';
        copyBtn.setAttribute('data-action', 'copy');
        copyBtn.setAttribute('data-message-id', messageId);

        const copyIcon = document.createElement('img');
        copyIcon.src = './assets/icons/main-content/messages/user/copy.svg';
        copyIcon.alt = 'Copy';
        copyIcon.className = 'user-message__action-icon';

        copyBtn.appendChild(copyIcon);
        // Text removed - only SVG icon

        // Add click listeners
        editBtn.addEventListener('click', () => {
            this.handleEdit(messageId, content);
        });

        copyBtn.addEventListener('click', () => {
            this.handleCopy(content);
        });

        actionsDiv.appendChild(editBtn);
        actionsDiv.appendChild(copyBtn);

        return actionsDiv;
    }

    /**
     * Add message to the chat
     * 
     * @param {string} content - The message content
     */
    addMessage(content) {
        if (!this.messagesContainer) return;

        const messageElement = this.createMessage(content);
        if (messageElement) {
            this.messagesContainer.appendChild(messageElement);
            
            // Scroll to bottom to show new message
            this.scrollToBottom();
        }
    }

    /**
     * Scroll messages container to bottom
     */
    scrollToBottom() {
        if (!this.messagesContainer) return;

        this.messagesContainer.scrollTop = this.messagesContainer.scrollHeight;
    }

    /* ========================================================================
       SHOW MORE/LESS TOGGLE
       ======================================================================== */

    /**
     * Toggle show more/less for a message
     * 
     * @param {string} messageId - The message ID
     */
    toggleShowMore(messageId) {
        const message = document.querySelector(`[data-message-id="${messageId}"]`);
        if (!message) return;

        const contentDiv = message.querySelector('.user-message__content');
        const showMoreBtn = message.querySelector('.user-message__show-more');

        if (!contentDiv || !showMoreBtn) return;

        const currentState = showMoreBtn.getAttribute('data-state');

        if (currentState === 'collapsed') {
            // Expand
            contentDiv.classList.remove('truncated');
            contentDiv.classList.add('expanded');
            showMoreBtn.textContent = 'Show less';
            showMoreBtn.setAttribute('data-state', 'expanded');
        } else {
            // Collapse
            contentDiv.classList.remove('expanded');
            contentDiv.classList.add('truncated');
            showMoreBtn.textContent = 'Show more';
            showMoreBtn.setAttribute('data-state', 'collapsed');
        }
    }

    /* ========================================================================
       ACTION HANDLERS
       ======================================================================== */

    /**
     * Handle edit action
     * 
     * Opens the message content in the input panel for editing.
     * 
     * @param {string} messageId - The message ID
     * @param {string} content - The message content
     */
    handleEdit(messageId, content) {
        console.log(`Edit message: ${messageId}`);

        // Get input panel controller
        if (window.inputPanelController) {
            window.inputPanelController.setMessage(content);
            window.inputPanelController.focus();
        }

        // TODO: In production, you might want to:
        // - Delete the message from the chat
        // - Track that this is an edit operation
        // - Update the message instead of creating a new one when sent
    }

    /**
     * Handle copy action
     * 
     * Copies the message content to clipboard.
     * 
     * @param {string} content - The message content
     */
    async handleCopy(content) {
        try {
            // Use Clipboard API if available
            if (navigator.clipboard && navigator.clipboard.writeText) {
                await navigator.clipboard.writeText(content);
                console.log('Message copied to clipboard');
                
                // TODO: Show a brief "Copied!" notification
                alert('Message copied to clipboard!');
            } else {
                // Fallback for older browsers
                this.fallbackCopyToClipboard(content);
            }
        } catch (error) {
            console.error('Failed to copy message:', error);
            alert('Failed to copy message');
        }
    }

    /**
     * Fallback copy to clipboard method
     * 
     * @param {string} text - The text to copy
     */
    fallbackCopyToClipboard(text) {
        const textArea = document.createElement('textarea');
        textArea.value = text;
        textArea.style.position = 'fixed';
        textArea.style.left = '-9999px';
        document.body.appendChild(textArea);
        textArea.select();
        
        try {
            document.execCommand('copy');
            console.log('Message copied to clipboard (fallback)');
            alert('Message copied to clipboard!');
        } catch (error) {
            console.error('Fallback copy failed:', error);
            alert('Failed to copy message');
        }
        
        document.body.removeChild(textArea);
    }

    /* ========================================================================
       PUBLIC API METHODS
       ======================================================================== */

    /**
     * Clear all messages
     */
    clearMessages() {
        if (!this.messagesContainer) return;

        this.messagesContainer.innerHTML = '';
        this.messageIdCounter = 0;
    }

    /**
     * Get message count
     * 
     * @returns {number} The number of messages
     */
    getMessageCount() {
        if (!this.messagesContainer) return 0;

        return this.messagesContainer.querySelectorAll('.user-message').length;
    }
}

/**
 * Initialize the user message controller when DOM is ready
 * 
 * This creates a single instance of the UserMessageController
 * and makes it globally accessible.
 */
let userMessageController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        userMessageController = new UserMessageController();
        // Make globally accessible
        window.userMessageController = userMessageController;
    });
} else {
    // DOM is already loaded
    userMessageController = new UserMessageController();
    window.userMessageController = userMessageController;
}

// Export for potential use in other modules
export default UserMessageController;
