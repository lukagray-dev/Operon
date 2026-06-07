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
 * Run highlight.js on all unhighlighted code blocks inside a container element.
 * Called after each assistant message chunk is fully rendered.
 * Uses the 'data-highlighted' attribute set by hljs to avoid double-processing.
 *
 * @param {HTMLElement} container - The .assistant-message__content element
 */
function highlightCodeBlocks(container) {
    if (typeof hljs !== 'undefined' && container) {
        // Query all pre elements inside the container to see if they need formatting
        container.querySelectorAll('pre').forEach(preEl => {
            const codeEl = preEl.querySelector('code');
            if (!codeEl) return;

            // Highlight the code block using Highlight.js if it hasn't been highlighted yet
            if (!codeEl.hasAttribute('data-highlighted')) {
                hljs.highlightElement(codeEl);
            }

            // Check if this pre block is already wrapped inside a code-block-wrapper.
            // If it is, we don't need to re-wrap or re-add headers.
            if (preEl.parentElement && preEl.parentElement.classList.contains('code-block-wrapper')) {
                return;
            }

            // Extract the programming language from classes (e.g., "language-javascript")
            let language = 'code';
            const classes = Array.from(codeEl.classList);
            const langClass = classes.find(cls => cls.startsWith('language-'));
            if (langClass) {
                language = langClass.replace('language-', '').toLowerCase();
                // Map common shortcuts to clean user-friendly labels
                const langMap = {
                    'js': 'javascript',
                    'ts': 'typescript',
                    'py': 'python',
                    'rs': 'rust',
                    'sh': 'shell',
                    'bash': 'shell',
                    'json': 'json',
                    'css': 'css',
                    'html': 'html',
                    'cpp': 'c++',
                    'cs': 'c#',
                    'go': 'go',
                    'rb': 'ruby'
                };
                if (langMap[language]) {
                    language = langMap[language];
                }
            }

            // Create our custom wrapper container to group the header bar and pre element
            const wrapper = document.createElement('div');
            wrapper.className = 'code-block-wrapper';

            // Create the top header bar to house the language text and the copy button
            const header = document.createElement('div');
            header.className = 'code-block-header';

            // Language name label
            const langSpan = document.createElement('span');
            langSpan.className = 'code-block-language';
            langSpan.textContent = language;

            // Copy button
            const copyBtn = document.createElement('button');
            copyBtn.className = 'code-block-copy-btn';
            copyBtn.setAttribute('title', 'Copy code');
            copyBtn.setAttribute('aria-label', 'Copy code');

            // Copy icon using the provided SVG path
            const copyIcon = document.createElement('img');
            copyIcon.src = './assets/icons/main-content/messages/assistant/copy.svg';
            copyIcon.className = 'code-block-copy-icon';
            copyBtn.appendChild(copyIcon);

            // Bind click handler to copy code block contents to clipboard
            copyBtn.addEventListener('click', () => {
                const codeText = codeEl.textContent;
                navigator.clipboard.writeText(codeText)
                    .then(() => {
                        // Apply 'copied' class to show micro-interaction feedback (visual checkmark/copied text)
                        copyBtn.classList.add('copied');
                        setTimeout(() => {
                            copyBtn.classList.remove('copied');
                        }, 2000);
                    })
                    .catch(err => {
                        console.error('Failed to copy code block:', err);
                    });
            });

            header.appendChild(langSpan);
            header.appendChild(copyBtn);

            // Re-organize the DOM by placing the wrapper right before the pre element,
            // then moving both the header and the pre element inside it.
            if (preEl.parentNode) {
                preEl.parentNode.insertBefore(wrapper, preEl);
                wrapper.appendChild(header);
                wrapper.appendChild(preEl);
            }
        });
    }
}

// Make the helper globally accessible so it can be invoked by the message controllers
window.highlightCodeBlocks = highlightCodeBlocks;

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
