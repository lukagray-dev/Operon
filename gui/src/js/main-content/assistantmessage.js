/**
 * Assistant Message Component
 * 
 * This module handles all functionality for assistant messages including:
 * - Rendering assistant messages in the chat area
 * - Action buttons (Copy, Like, Dislike, Regenerate, Fork)
 * - Time display
 * - No bubble design with PT Serif font
 * 
 * Assistant messages appear full-width matching the input panel edges.
 */

'use strict';

import { renderMarkdown } from '../shared/ipc.js';

/**
 * AssistantMessageController class
 * 
 * Manages assistant message rendering and actions.
 * This class follows the single responsibility principle by separating
 * concerns into focused private methods.
 */
class AssistantMessageController {
    /**
     * Constructor - Initializes the assistant message controller
     * 
     * Sets up:
     * - Reference to messages container
 * - Message ID counter
     * - Like/dislike state tracking
     */
    constructor() {
        // DOM element references
        this.messagesContainer = null;
        
        // Message ID counter for unique identification
        this.messageIdCounter = 0;
        
        // Track like/dislike states
        this.likeStates = new Map(); // messageId -> 'like', 'dislike', or null
        
        // Initialize
        this.init();
    }

    /**
     * Initialize the assistant message controller
     * 
     * Sets up the messages container reference.
     */
    init() {
        try {
            this.messagesContainer = document.querySelector('.main-content__messages');
            
            if (!this.messagesContainer) {
                throw new Error('Messages container not found');
            }
            
            console.log('Assistant message controller initialized successfully');
        } catch (error) {
            console.error('Failed to initialize assistant message controller:', error);
        }
    }

    /* ========================================================================
       MESSAGE RENDERING
       ======================================================================== */

    /**
     * Create and render an assistant message
     * 
     * @param {string|null} content - The message content. If null, we create an empty container for streaming sequential blocks.
     * @param {string} timestamp - Optional timestamp (e.g., "2m ago")
     * @returns {HTMLElement} The created message element
     */
    createMessage(content, timestamp = null) {
        // Generate unique message ID to keep track of this specific message
        const messageId = `assistant-message-${this.messageIdCounter++}`;

        // Create the main wrapper container for the assistant message
        const messageDiv = document.createElement('div');
        messageDiv.className = 'assistant-message';
        messageDiv.setAttribute('data-message-id', messageId);

        // We only create and append the content div if content is NOT null.
        // If content is null, we are starting a stream or rendering sequential history,
        // and we will append text blocks, thinking blocks, and tool cards on the fly.
        if (content !== null) {
            const contentDiv = document.createElement('div');
            contentDiv.className = 'assistant-message__content markdown-content';
            
            // If content is not empty, render it asynchronously as markdown.
            // This prevents the page from freezing up for long messages.
            if (content && content.trim() !== "") {
                contentDiv.rawMarkdown = content;
                renderMarkdown(content)
                    .then(html => {
                        contentDiv.innerHTML = html;
                        // Auto-typeset math equations using KaTeX if the library is loaded in the browser
                        if (window.renderMathInElement) {
                            window.renderMathInElement(contentDiv, {
                                delimiters: [
                                    {left: '$$', right: '$$', display: true},
                                    {left: '$', right: '$', display: false},
                                    {left: '\\(', right: '\\)', display: false},
                                    {left: '\\[', right: '\\]', display: true}
                                ],
                                throwOnError: false
                            });
                        }
                        // Apply syntax highlighting to code blocks in the loaded message
                        if (window.highlightCodeBlocks) {
                            window.highlightCodeBlocks(contentDiv);
                        }
                    })
                    .catch(err => {
                        console.error("Failed to render markdown on history load:", err);
                        contentDiv.textContent = content;
                    });
            } else {
                contentDiv.textContent = content;
            }
            messageDiv.appendChild(contentDiv);
        }

        // Create separator line (the horizontal line separating content and actions)
        const separatorDiv = document.createElement('div');
        separatorDiv.className = 'assistant-message__separator';
        messageDiv.appendChild(separatorDiv);

        // Create actions row (like/dislike/copy/regenerate buttons) at the bottom
        const actionsDiv = this.createActionsRow(messageId, content || "", timestamp);
        messageDiv.appendChild(actionsDiv);

        return messageDiv;
    }

    /**
     * Create actions row
     * 
     * @param {string} messageId - The message ID
     * @param {string} content - The message content
     * @param {string} timestamp - Optional timestamp
     * @returns {HTMLElement} The actions container
     */
    createActionsRow(messageId, content, timestamp) {
        const actionsDiv = document.createElement('div');
        actionsDiv.className = 'assistant-message__actions';

        // 1. Operon logo button
        const logoBtn = this.createLogoButton();
        actionsDiv.appendChild(logoBtn);

        // 2. Copy button
        const copyBtn = this.createActionButton('copy', 'Copy', messageId, content);
        actionsDiv.appendChild(copyBtn);

        // 3. Like button
        const likeBtn = this.createActionButton('like', 'Like', messageId, content);
        actionsDiv.appendChild(likeBtn);

        // 4. Dislike button (grouped with like)
        const dislikeBtn = this.createActionButton('dislike', 'Dislike', messageId, content);
        actionsDiv.appendChild(dislikeBtn);

        // 5. Fork button
        const forkBtn = this.createActionButton('fork', 'Fork', messageId, content);
        actionsDiv.appendChild(forkBtn);

        // 6. Time (if provided)
        if (timestamp) {
            const timeSpan = document.createElement('span');
            timeSpan.className = 'assistant-message__time';
            timeSpan.textContent = timestamp;
            actionsDiv.appendChild(timeSpan);
        }

        return actionsDiv;
    }

    /**
     * Create logo button
     * 
     * @returns {HTMLElement} The logo button
     */
    createLogoButton() {
        const button = document.createElement('button');
        button.className = 'assistant-message__action-btn assistant-message__logo-btn';
        button.setAttribute('aria-label', 'Operon');
        button.setAttribute('title', 'Operon');

        const icon = document.createElement('img');
        icon.src = './assets/brand/operon.svg';
        icon.alt = 'Operon';
        icon.className = 'assistant-message__logo-icon';

        button.appendChild(icon);

        // Add click listener (could show info about Operon)
        button.addEventListener('click', () => {
            console.log('Operon logo clicked');
            // TODO: Show Operon info or model details
        });

        return button;
    }

    /**
     * Create action button
     * 
     * @param {string} action - The action type
     * @param {string} label - The button label
     * @param {string} messageId - The message ID
     * @param {string} content - The message content
     * @returns {HTMLElement} The action button
     */
    createActionButton(action, label, messageId, content) {
        const button = document.createElement('button');
        button.className = 'assistant-message__action-btn';
        button.setAttribute('data-action', action);
        button.setAttribute('data-message-id', messageId);
        button.setAttribute('aria-label', label);

        // Icon mapping
        const iconMap = {
            'copy': 'copy.svg',
            'like': 'like.svg',
            'dislike': 'dislike.svg',
            'fork': 'fork.svg'
        };

        const icon = document.createElement('img');
        icon.src = `./assets/icons/main-content/messages/assistant/${iconMap[action]}`;
        icon.alt = label;
        icon.className = 'assistant-message__action-icon';

        button.appendChild(icon);
        // Text removed - only SVG icon

        // Add click listener
        button.addEventListener('click', () => {
            this.handleAction(action, messageId, content, button);
        });

        return button;
    }

    /**
     * Create dot separator
     * 
     * @returns {HTMLElement} The dot separator
     */
    createDotSeparator() {
        const dot = document.createElement('div');
        dot.className = 'assistant-message__dot';
        return dot;
    }

    /**
     * Add message to the chat
     * 
     * @param {string} content - The message content
     * @param {string} timestamp - Optional timestamp
     */
    addMessage(content, timestamp = null) {
        if (!this.messagesContainer) return;

        const messageElement = this.createMessage(content, timestamp);
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
       ACTION HANDLERS
       ======================================================================== */

    /**
     * Handle action button click
     * 
     * @param {string} action - The action type
     * @param {string} messageId - The message ID
     * @param {string} content - The message content
     * @param {HTMLElement} button - The button element
     */
    handleAction(action, messageId, content, button) {
        // Find the message DOM element and extract its accumulated raw markdown from all content segments.
        // Since the response can be split into multiple text content blocks interspersed with thinking and tool calls,
        // we gather text from all of them to make copy/fork actions work correctly with the whole message.
        const messageEl = document.querySelector(`[data-message-id="${messageId}"]`);
        let actualContent = "";
        
        if (messageEl) {
            const contentEls = messageEl.querySelectorAll('.assistant-message__content');
            if (contentEls.length > 0) {
                const parts = [];
                contentEls.forEach(el => {
                    if (el.rawMarkdown !== undefined && el.rawMarkdown !== null) {
                        parts.push(el.rawMarkdown);
                    } else if (el.textContent) {
                        parts.push(el.textContent);
                    }
                });
                actualContent = parts.join("\n\n");
            } else {
                actualContent = content;
            }
        } else {
            actualContent = content;
        }

        switch (action) {
            case 'copy':
                this.handleCopy(actualContent, button);
                break;
            case 'like':
                this.handleLike(messageId, button);
                break;
            case 'dislike':
                this.handleDislike(messageId, button);
                break;
            case 'fork':
                this.handleFork(messageId, actualContent);
                break;
            default:
                console.warn(`Unknown action: ${action}`);
        }
    }

    async handleCopy(content, button) {
        try {
            if (navigator.clipboard && navigator.clipboard.writeText) {
                await navigator.clipboard.writeText(content);
                console.log('Message copied to clipboard');
                this.showCopyDoneIcon(button);
            } else {
                this.fallbackCopyToClipboard(content, button);
            }
        } catch (error) {
            console.error('Failed to copy message:', error);
        }
    }

    /**
     * Fallback copy to clipboard method
     * 
     * @param {string} text - The text to copy
     * @param {HTMLElement} button - The copy button element
     */
    fallbackCopyToClipboard(text, button) {
        const textArea = document.createElement('textarea');
        textArea.value = text;
        textArea.style.position = 'fixed';
        textArea.style.left = '-9999px';
        document.body.appendChild(textArea);
        textArea.select();
        
        try {
            document.execCommand('copy');
            console.log('Message copied to clipboard (fallback)');
            this.showCopyDoneIcon(button);
        } catch (error) {
            console.error('Fallback copy failed:', error);
        }
        
        document.body.removeChild(textArea);
    }

    /**
     * Handle like action
     * 
     * @param {string} messageId - The message ID
     * @param {HTMLElement} button - The like button
     */
    handleLike(messageId, button) {
        const currentState = this.likeStates.get(messageId);
        const message = document.querySelector(`[data-message-id="${messageId}"]`);
        
        if (!message) return;

        const dislikeBtn = message.querySelector('[data-action="dislike"]');

        if (currentState === 'like') {
            // Unliking
            button.classList.remove('active');
            this.likeStates.set(messageId, null);
            console.log('Unliked message:', messageId);
        } else {
            // Liking
            button.classList.add('active');
            if (dislikeBtn) dislikeBtn.classList.remove('active');
            this.likeStates.set(messageId, 'like');
            console.log('Liked message:', messageId);
            // TODO: Send feedback to backend
        }
    }

    /**
     * Handle dislike action
     * 
     * @param {string} messageId - The message ID
     * @param {HTMLElement} button - The dislike button
     */
    handleDislike(messageId, button) {
        const currentState = this.likeStates.get(messageId);
        const message = document.querySelector(`[data-message-id="${messageId}"]`);
        
        if (!message) return;

        const likeBtn = message.querySelector('[data-action="like"]');

        if (currentState === 'dislike') {
            // Undoing dislike
            button.classList.remove('active');
            this.likeStates.set(messageId, null);
            console.log('Removed dislike from message:', messageId);
        } else {
            // Disliking
            button.classList.add('active');
            if (likeBtn) likeBtn.classList.remove('active');
            this.likeStates.set(messageId, 'dislike');
            console.log('Disliked message:', messageId);
            // TODO: Send feedback to backend
        }
    }

    /**
     * Show a copy-done SVG checkmark icon on the copy button for 5 seconds.
     *
     * Hey friend! This method temporarily swaps the copy button's icon source to copy-done.svg
     * and sets a timeout to change it back to copy.svg after 5 seconds. This provides a sleek,
     * modern micro-interaction feedback.
     *
     * @param {HTMLElement} button - The copy button element
     */
    showCopyDoneIcon(button) {
        if (!button) return;
        const img = button.querySelector('img');
        if (!img) return;

        // Save original source
        const originalSrc = img.src;
        
        // Swap to copy-done
        img.src = './assets/icons/main-content/messages/assistant/copy-done.svg';

        // Clear any existing timeout on this button to avoid overlapping swaps
        if (button.copyTimeout) {
            clearTimeout(button.copyTimeout);
        }

        // Revert back after 5 seconds (5000 milliseconds)
        button.copyTimeout = setTimeout(() => {
            img.src = './assets/icons/main-content/messages/assistant/copy.svg';
            button.copyTimeout = null;
        }, 5000);
    }

    /**
     * Handle fork action
     * 
     * @param {string} messageId - The message ID
     * @param {string} content - The message content
     */
    handleFork(messageId, content) {
        console.log('Fork conversation at message:', messageId);
        // TODO: Implement conversation forking
        // - Create a new conversation branch from this point
        // - Allow exploring alternative responses
        alert('Fork functionality will be implemented here');
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
        this.likeStates.clear();
    }

    /**
     * Get message count
     * 
     * @returns {number} The number of messages
     */
    getMessageCount() {
        if (!this.messagesContainer) return 0;

        return this.messagesContainer.querySelectorAll('.assistant-message').length;
    }
}

/**
 * Initialize the assistant message controller when DOM is ready
 * 
 * This creates a single instance of the AssistantMessageController
 * and makes it globally accessible.
 */
let assistantMessageController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        assistantMessageController = new AssistantMessageController();
        // Make globally accessible
        window.assistantMessageController = assistantMessageController;
    });
} else {
    // DOM is already loaded
    assistantMessageController = new AssistantMessageController();
    window.assistantMessageController = assistantMessageController;
}

// Export for potential use in other modules
export default AssistantMessageController;
