/**
 * Input Panel Component
 * 
 * This module handles all functionality for the input panel including:
 * - Textarea auto-expanding (1-10 lines)
 * - Focus/blur state management (border and glow)
 * - Action button handlers (Attach, Auto-approve, Context, Model, Reasoning, Voice, Send)
 * - Message sending
 * - Keyboard shortcuts (Enter to send, Shift+Enter for new line)
 * 
 * The input panel floats at the bottom of the main content area and provides
 * the primary interface for user input.
 */

'use strict';

/**
 * InputPanelController class
 * 
 * Manages all input panel interactions including text input, auto-expansion,
 * focus states, and action buttons. This class follows the single responsibility
 * principle by separating concerns into focused private methods.
 */
class InputPanelController {
    /**
     * Constructor - Initializes the input panel controller
     * 
     * Sets up:
     * - References to DOM elements
     * - State tracking (focus, auto-approve)
     * - Event listeners for all interactive elements
     * - Sidebar resize observer
     */
    constructor() {
        // DOM element references
        this.container = null;
        this.textarea = null;
        this.autoApproveBtn = null;
        
        // State tracking
        this.isFocused = false;
        this.autoApproveEnabled = false;
        this._activeDropdown = null;
        
        // Textarea settings
        this.lineHeight = 20; // Must match CSS --input-panel-textarea-line-height
        this.maxLines = 10;
        this.maxHeight = this.lineHeight * this.maxLines;
        
        // Initialize the input panel
        this.init();
    }

    /**
     * Initialize all input panel functionality
     * 
     * This is the main entry point that sets up all event listeners
     * and initializes the textarea state.
     */
    init() {
        try {
            // Get DOM element references
            this.container = document.querySelector('.input-panel__container');
            this.textarea = document.getElementById('input-textarea');
            this.autoApproveBtn = document.getElementById('input-auto-approve');
            
            if (!this.container || !this.textarea) {
                throw new Error('Required input panel elements not found');
            }
            
            // Set up all event listeners
            this.initEventListeners();
            
            // Setup sidebar resize observer
            this.observeSidebarResize();
            
            console.log('Input panel initialized successfully');
        } catch (error) {
            console.error('Failed to initialize input panel:', error);
        }
    }

    /**
     * Observe sidebar resize to update input panel position
     * 
     * Uses ResizeObserver to watch for sidebar width changes
     * and adjusts input panel positioning accordingly.
     */
    observeSidebarResize() {
        const sidebar = document.querySelector('.left-sidebar');
        const inputPanel = document.querySelector('.input-panel');
        
        if (!sidebar || !inputPanel) return;

        // Create a ResizeObserver to watch sidebar width changes
        const resizeObserver = new ResizeObserver(entries => {
            for (const entry of entries) {
                const sidebarWidth = entry.contentRect.width;
                // Update input panel left position to match sidebar width
                inputPanel.style.left = `${sidebarWidth}px`;
            }
        });

        // Start observing the sidebar
        resizeObserver.observe(sidebar);
    }

    /**
     * Initialize all event listeners
     * 
     * Sets up listeners for:
     * - Textarea input, focus, blur, keydown
     * - Action buttons
     */
    initEventListeners() {
        // Textarea events
        this.setupTextareaListeners();
        
        // Action button events
        this.setupActionButtons();
    }

    /* ========================================================================
       TEXTAREA HANDLERS
       ======================================================================== */

    /**
     * Setup textarea event listeners
     * 
     * Handles:
     * - Auto-expansion on input
     * - Focus/blur states (border and glow)
     * - Keyboard shortcuts
     */
    setupTextareaListeners() {
        if (!this.textarea) return;

        // Input event - handle auto-expansion
        this.textarea.addEventListener('input', () => {
            this.adjustTextareaHeight();
        });

        // Focus event - add focused state to container
        this.textarea.addEventListener('focus', () => {
            this.handleFocus();
        });

        // Blur event - remove focused state from container
        this.textarea.addEventListener('blur', () => {
            this.handleBlur();
        });

        // Keydown event - handle Enter key
        this.textarea.addEventListener('keydown', (e) => {
            this.handleKeyDown(e);
        });
    }

    /**
     * Adjust textarea height based on content
     * 
     * Expands from 1 line to max 10 lines, then scrolls.
     * Uses scrollHeight to calculate required height.
     * Also updates send button state based on text content.
     */
    adjustTextareaHeight() {
        if (!this.textarea) return;

        // Reset height to auto to get accurate scrollHeight
        this.textarea.style.height = 'auto';

        // Calculate new height based on content
        const scrollHeight = this.textarea.scrollHeight;
        
        // Constrain height between min (1 line) and max (10 lines)
        const minHeight = this.lineHeight;
        const newHeight = Math.min(Math.max(scrollHeight, minHeight), this.maxHeight);

        // Apply new height
        this.textarea.style.height = `${newHeight}px`;

        // Update send button state based on text content
        this.updateSendButtonState();
    }

    /**
     * Handle textarea focus
     * 
     * Adds focused class to container for border and glow effect.
     */
    handleFocus() {
        if (!this.container) return;
        
        this.isFocused = true;
        this.container.classList.add('focused');
    }

    /**
     * Handle textarea blur
     * 
     * Removes focused class from container.
     */
    handleBlur() {
        if (!this.container) return;
        
        this.isFocused = false;
        this.container.classList.remove('focused');
    }

    /**
     * Handle keydown events in textarea
     * 
     * Keyboard shortcuts:
     * - Enter: Send message (if not empty)
     * - Shift+Enter: New line
     * 
     * @param {KeyboardEvent} e - The keyboard event
     */
    handleKeyDown(e) {
        // Enter key without Shift - send message
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            this.sendMessage();
        }
        
        // Shift+Enter - allow default (new line)
    }

    /* ========================================================================
       ACTION BUTTON HANDLERS
       ======================================================================== */

    /**
     * Setup action button listeners
     * 
     * Handles clicks on all action buttons:
     * - Attach
     * - Auto-approve (toggle)
     * - Context
     * - Model (dropdown)
     * - Reasoning
     * - Voice
     * - Send
     */
    setupActionButtons() {
        // Attach button
        const attachBtn = document.getElementById('input-attach');
        if (attachBtn) {
            attachBtn.addEventListener('click', () => {
                this.handleAttach();
            });
        }

        // Auto-approve button (toggle)
        if (this.autoApproveBtn) {
            this.autoApproveBtn.addEventListener('click', () => {
                this.toggleAutoApprove();
            });
        }

        // Context button
        const contextBtn = document.getElementById('input-context');
        if (contextBtn) {
            contextBtn.addEventListener('click', () => {
                this.handleContext();
            });
        }

        // Model button (dropdown)
        const modelBtn = document.getElementById('input-model');
        if (modelBtn) {
            modelBtn.addEventListener('click', () => {
                this.handleModelSelect();
            });
        }

        // Reasoning button
        const reasoningBtn = document.getElementById('input-reasoning');
        if (reasoningBtn) {
            reasoningBtn.addEventListener('click', () => {
                this.handleReasoning();
            });
        }

        // Voice button
        const voiceBtn = document.getElementById('input-voice');
        if (voiceBtn) {
            voiceBtn.addEventListener('click', () => {
                this.handleVoice();
            });
        }

        // Send button
        const sendBtn = document.getElementById('input-send');
        if (sendBtn) {
            sendBtn.addEventListener('click', () => {
                this.sendMessage();
            });
        }
    }

    /**
     * Handle attach button click
     * 
     * Opens file picker to attach files to the message.
     * In production, this would:
     * - Open native file picker dialog
     * - Handle file upload
     * - Display attached files in the input panel
     */
    handleAttach() {
        console.log('Attach clicked');
        // TODO: Implement file attachment
        alert('Attach file functionality will be implemented here');
    }

    /**
     * Toggle auto-approve state
     * 
     * Switches between enabled/disabled states and updates the icon.
     */
    toggleAutoApprove() {
        if (!this.autoApproveBtn) return;

        this.autoApproveEnabled = !this.autoApproveEnabled;
        
        // Update button state attribute
        this.autoApproveBtn.setAttribute(
            'data-state', 
            this.autoApproveEnabled ? 'enabled' : 'disabled'
        );

        console.log(`Auto-approve ${this.autoApproveEnabled ? 'enabled' : 'disabled'}`);
    }

    /**
     * Handle context button click
     * 
     * Opens context selection interface.
     * In production, this would:
     * - Show context picker (files, folders, URLs)
     * - Allow adding context to the message
     */
    handleContext() {
        console.log('Context clicked');
        // TODO: Implement context selection
        alert('Context selection functionality will be implemented here');
    }

    /**
     * Handle model selection button click
     * 
     * Opens model selector dropdown showing available models from the active provider.
     */
    async handleModelSelect() {
        console.log('Model selector clicked');
        
        // Import IPC module dynamically
        const IPC = await import('../shared/ipc.js');
        
        try {
            // Get active provider and available models
            const activeProvider = await IPC.getActiveProvider();
            
            if (!activeProvider) {
                alert('No active provider configured. Please configure a model provider in Settings.');
                return;
            }
            
            // Show model selection dropdown
            this.showModelDropdown(activeProvider);
        } catch (error) {
            console.error('Failed to load models:', error);
            alert('Failed to load available models. Please try again.');
        }
    }
    
    /**
     * Show model selection dropdown
     * 
     * @param {Object} providerSetup - The active provider configuration
     */
    showModelDropdown(providerSetup) {
        const modelBtn = document.getElementById('input-model');
        if (!modelBtn) return;
        
        // Remove existing dropdown if any
        this.closeModelDropdown();
        
        // Get available models (discovered or fallback)
        const availableModels = providerSetup.fallbackModels || [];
        const currentModel = providerSetup.selectedModel || availableModels[0] || '';
        
        if (availableModels.length === 0) {
            alert('No models available for this provider. Please configure models in Settings.');
            return;
        }
        
        // Create dropdown HTML
        const dropdown = document.createElement('div');
        dropdown.className = 'model-selector__dropdown';
        dropdown.innerHTML = `
            <div class="model-selector__header">
                <span class="model-selector__title">${this.escapeHtml(providerSetup.label)}</span>
                <span class="model-selector__subtitle">Select Model</span>
            </div>
            <div class="model-selector__list">
                ${availableModels.map(modelId => `
                    <button 
                        class="model-selector__item ${modelId === currentModel ? 'is-active' : ''}"
                        data-model-id="${this.escapeHtml(modelId)}"
                    >
                        <span class="model-selector__item-name">${this.escapeHtml(modelId)}</span>
                        ${modelId === currentModel ? '<span class="model-selector__item-check">✓</span>' : ''}
                    </button>
                `).join('')}
            </div>
            <div class="model-selector__footer">
                <span class="model-selector__context">Context: ${this.formatContextWindow(providerSetup.selectedModel)}</span>
            </div>
        `;
        
        // Position dropdown
        const btnRect = modelBtn.getBoundingClientRect();
        dropdown.style.position = 'fixed';
        dropdown.style.bottom = `${window.innerHeight - btnRect.top + 8}px`;
        dropdown.style.right = `${window.innerWidth - btnRect.right}px`;
        
        // Add to body
        document.body.appendChild(dropdown);
        
        // Bind events
        dropdown.querySelectorAll('.model-selector__item').forEach(item => {
            item.addEventListener('click', async () => {
                const modelId = item.getAttribute('data-model-id');
                await this.selectModel(providerSetup.providerId, modelId);
                this.closeModelDropdown();
            });
        });
        
        // Click outside to close
        setTimeout(() => {
            document.addEventListener('click', this.handleDropdownClickOutside.bind(this), { once: true });
        }, 0);
        
        // Store reference
        this._activeDropdown = dropdown;
    }
    
    /**
     * Close model dropdown
     */
    closeModelDropdown() {
        if (this._activeDropdown) {
            this._activeDropdown.remove();
            this._activeDropdown = null;
        }
    }
    
    /**
     * Handle click outside dropdown
     * 
     * @param {MouseEvent} e - Click event
     */
    handleDropdownClickOutside(e) {
        const modelBtn = document.getElementById('input-model');
        if (!modelBtn) return;
        
        if (!modelBtn.contains(e.target) && this._activeDropdown && !this._activeDropdown.contains(e.target)) {
            this.closeModelDropdown();
        }
    }
    
    /**
     * Select a model
     * 
     * @param {string} providerId - Provider ID
     * @param {string} modelId - Model ID to select
     */
    async selectModel(providerId, modelId) {
        try {
            // Import IPC module
            const IPC = await import('../shared/ipc.js');
            
            // Get current provider setup
            const setup = await IPC.getModelProviderSetup(providerId);
            
            // Save with new model
            await IPC.saveProviderSetup({
                providerId: providerId,
                apiBase: setup.apiBase || setup.defaultApiBase,
                apiKey: setup.apiKey,
                model: modelId,
            });
            
            // Update UI to show selected model
            this.updateModelDisplay(modelId);
            
            console.log(`Model changed to: ${modelId}`);
        } catch (error) {
            console.error('Failed to select model:', error);
            alert('Failed to change model. Please try again.');
        }
    }
    
    /**
     * Update model display in button
     * 
     * @param {string} modelId - The model ID to display
     */
    updateModelDisplay(modelId) {
        const modelBtn = document.getElementById('input-model');
        const labelEl = modelBtn?.querySelector('.input-panel__action-label');
        
        if (labelEl) {
            // Extract a friendly name from the model ID
            let displayName = modelId;
            
            // Try to extract a short name
            if (modelId.includes('sonnet')) displayName = 'Sonnet';
            else if (modelId.includes('opus')) displayName = 'Opus';
            else if (modelId.includes('haiku')) displayName = 'Haiku';
            else if (modelId.includes('gpt-4o-mini')) displayName = 'GPT-4o Mini';
            else if (modelId.includes('gpt-4o')) displayName = 'GPT-4o';
            else if (modelId.includes('o4-mini')) displayName = 'o4 Mini';
            else if (modelId.includes('o3')) displayName = 'o3';
            else if (modelId.includes('gemini')) displayName = 'Gemini';
            else if (modelId.includes('deepseek')) displayName = 'DeepSeek';
            else if (modelId.includes('llama')) displayName = 'Llama';
            
            labelEl.textContent = displayName;
        }
    }
    
    /**
     * Format context window size
     * 
     * @param {string} modelId - Model ID
     * @returns {string} Formatted context window
     */
    formatContextWindow(modelId) {
        // TODO: Get actual context window from backend
        // For now, return placeholder
        return '128K';
    }
    
    /**
     * Escape HTML to prevent XSS
     * 
     * @param {string} str - String to escape
     * @returns {string} Escaped string
     */
    escapeHtml(str) {
        const div = document.createElement('div');
        div.textContent = str;
        return div.innerHTML;
    }

    /**
     * Handle reasoning button click
     * 
     * Toggles reasoning mode for the AI.
     * In production, this would:
     * - Enable/disable extended reasoning
     * - Update UI to show reasoning is enabled
     */
    handleReasoning() {
        console.log('Reasoning clicked');
        // TODO: Implement reasoning toggle
        alert('Reasoning functionality will be implemented here');
    }

    /**
     * Handle voice button click
     * 
     * Starts voice input recording.
     * In production, this would:
     * - Request microphone permissions
     * - Start recording audio
     * - Transcribe audio to text
     * - Insert transcription into textarea
     */
    handleVoice() {
        console.log('Voice clicked');
        // TODO: Implement voice input
        alert('Voice input functionality will be implemented here');
    }

    /**
     * Update send button state based on text content
     * 
     * Adds 'active' class when there's text in the textarea,
     * removes it when empty.
     */
    updateSendButtonState() {
        const sendBtn = document.getElementById('input-send');
        if (!sendBtn || !this.textarea) return;

        const hasText = this.textarea.value.trim().length > 0;

        if (hasText) {
            sendBtn.classList.add('active');
        } else {
            sendBtn.classList.remove('active');
        }
    }

    /* ========================================================================
       MESSAGE SENDING
       ======================================================================== */

    /**
     * Send message
     * 
     * Validates and sends the message content.
     * Creates a user message in the chat display.
     * Simulates an assistant response for testing.
     */
    sendMessage() {
        if (!this.textarea) return;

        const message = this.textarea.value.trim();

        // Don't send empty messages
        if (!message) {
            console.log('Cannot send empty message');
            return;
        }

        console.log('Sending message:', message);
        console.log('Auto-approve enabled:', this.autoApproveEnabled);

        // Hide empty state if visible
        if (window.emptyStateController) {
            window.emptyStateController.hideEmptyState();
        }

        // Create user message in chat
        if (window.userMessageController) {
            window.userMessageController.addMessage(message);
        }

        // Clear textarea after sending
        this.clearInput();

        // Simulate assistant response immediately (for testing)
        if (window.assistantMessageController) {
            const mockResponse = `This is a sample assistant message using PT Serif font. The assistant's response appears without a bubble, spanning the full width of the input panel. Below this message, you'll find action buttons for copying, liking, regenerating, and more.`;
            window.assistantMessageController.addMessage(mockResponse, '2m ago');
        }

        // TODO: Implement backend message sending
        // - Send to backend API
        // - Get AI response
        // - Display assistant message
    }

    /**
     * Clear input textarea
     * 
     * Resets textarea content and height to initial state.
     * Also removes active state from send button.
     */
    clearInput() {
        if (!this.textarea) return;

        this.textarea.value = '';
        this.textarea.style.height = 'auto';
        
        // Adjust height to default (1 line)
        this.adjustTextareaHeight();

        // Remove active state from send button
        this.updateSendButtonState();
    }

    /* ========================================================================
       PUBLIC API METHODS
       ======================================================================== */

    /**
     * Set message text programmatically
     * 
     * @param {string} text - The text to set in the textarea
     */
    setMessage(text) {
        if (!this.textarea) return;

        this.textarea.value = text;
        this.adjustTextareaHeight();
    }

    /**
     * Get current message text
     * 
     * @returns {string} The current textarea content
     */
    getMessage() {
        return this.textarea ? this.textarea.value : '';
    }

    /**
     * Focus the textarea
     */
    focus() {
        if (this.textarea) {
            this.textarea.focus();
        }
    }

    /**
     * Get auto-approve state
     * 
     * @returns {boolean} Whether auto-approve is enabled
     */
    isAutoApproveEnabled() {
        return this.autoApproveEnabled;
    }

    /**
     * Set auto-approve state programmatically
     * 
     * @param {boolean} enabled - Whether to enable auto-approve
     */
    setAutoApprove(enabled) {
        if (!this.autoApproveBtn) return;

        this.autoApproveEnabled = enabled;
        this.autoApproveBtn.setAttribute(
            'data-state', 
            enabled ? 'enabled' : 'disabled'
        );
    }
}

/**
 * Initialize the input panel when DOM is ready
 * 
 * This creates a single instance of the InputPanelController
 * and makes it globally accessible for debugging and external use.
 */
let inputPanelController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        inputPanelController = new InputPanelController();
        // Make globally accessible
        window.inputPanelController = inputPanelController;
    });
} else {
    // DOM is already loaded
    inputPanelController = new InputPanelController();
    window.inputPanelController = inputPanelController;
}

// Export for potential use in other modules
export default InputPanelController;
