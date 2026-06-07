'use strict';

/**
 * session-manager.js
 *
 * Central frontend controller for managing agent chat sessions.
 * Coordinates between left-sidebar list, input panel, user messages,
 * assistant messages, and Tauri IPC commands.
 *
 * It listens to "session-event" from Rust to stream live responses,
 * thinking blocks, tool calls, and permission prompts.
 */

import * as IPC from './ipc.js';
import { showError, showSuccess } from './toast.js';

class SessionManager {
    constructor() {
        this.activeSessionId = null;
        this.currentProjectDir = null; // VS Code style opened project path
        this.sessions = [];            // Cache list of session items from backend
        
        // Element references for streaming the current response
        this.currentAssistantMsgEl = null;
        this.currentAssistantContentEl = null;
        
        // Reasoning block variables
        this.currentThinkingEl = null;
        this.currentThinkingContentEl = null;
        
        // Maps to track active inline components by ID
        this.activeToolCalls = new Map();         // call_id -> card DOM element
        this.activePermissionPrompts = new Map(); // approval_id -> card DOM element
        this.activeAskPrompts = new Map();        // ask_id -> card DOM element
        
        // Tauri unlisten handle
        this.unlistenSessionEvents = null;
    }

    /**
     * Initialize the session manager
     */
    async init() {
        try {
            // Bind Tauri event listener for live streaming
            if (window.__TAURI__) {
                const { listen } = window.__TAURI__.event;
                this.unlistenSessionEvents = await listen('session-event', (event) => {
                    this.handleSessionEvent(event.payload);
                });
            }
            
            console.log('Session manager initialized successfully');
            
            // Load initial chats list in the sidebar
            await this.loadSessionsList();
        } catch (error) {
            console.error('Failed to initialize session manager:', error);
        }
    }

    /**
     * Terminate and clean up Tauri event listener
     */
    destroy() {
        if (this.unlistenSessionEvents) {
            this.unlistenSessionEvents();
            this.unlistenSessionEvents = null;
        }
    }

    /**
     * Reset streaming references for a new turn
     */
    resetStreamingState() {
        this.hideTypingIndicator();
        this.currentAssistantMsgEl = null;
        this.currentAssistantContentEl = null;
        this.currentThinkingEl = null;
        this.currentThinkingContentEl = null;
        this.activeToolCalls.clear();
        this.activePermissionPrompts.clear();
        this.activeAskPrompts.clear();
    }

    /**
     * Show a bouncing typing indicator at the end of the chat container
     */
    showTypingIndicator() {
        this.hideTypingIndicator(); // Ensure any previous one is cleaned up
        
        const container = window.assistantMessageController?.messagesContainer;
        if (!container) return;
        
        const indicator = document.createElement('div');
        indicator.className = 'assistant-message__typing-indicator';
        indicator.id = 'assistant-typing-indicator';
        indicator.innerHTML = `
            <div class="assistant-message__typing-dot"></div>
            <div class="assistant-message__typing-dot"></div>
            <div class="assistant-message__typing-dot"></div>
        `;
        
        container.appendChild(indicator);
        window.assistantMessageController.scrollToBottom();
    }

    /**
     * Remove the typing indicator from the DOM
     */
    hideTypingIndicator() {
        const indicator = document.getElementById('assistant-typing-indicator');
        if (indicator) {
            indicator.remove();
        }
    }

    /**
     * Start a brand new, empty chat session.
     * Clears messages, hides empty state, and updates the title.
     *
     * Hey friend! We've updated this function to accept an optional `projectDir` parameter.
     * When starting a new chat:
     * - If `projectDir` is provided, we bind the new chat session to that project path.
     * - If `projectDir` is null/omitted, it will be a normal standalone chat session.
     *
     * @param {string|null} [projectDir=null] - Optional project path to create a project-specific chat
     */
    startNewChat(projectDir = null) {
        this.activeSessionId = null;
        this.currentProjectDir = projectDir;
        this.resetStreamingState();
        
        // Clear diagnostics bar if visible
        if (window.inputPanelController) {
            window.inputPanelController.clearDiagnostics();
            // Reset context usage display back to default placeholder values when starting a fresh session
            window.inputPanelController.updateContextUsage(0, 0, 0);
        }
        
        // Clear message log
        if (window.userMessageController) {
            window.userMessageController.clearMessages();
        }
        if (window.assistantMessageController) {
            window.assistantMessageController.clearMessages();
        }
        
        // Restore empty state
        if (window.emptyStateController) {
            window.emptyStateController.showEmptyState();
        }
        
        // Update session header title
        const titleEl = document.getElementById('session-title');
        if (titleEl) {
            titleEl.textContent = 'New Chat';
        }
        
        // Deselect any active item in left sidebar
        const activeItem = document.querySelector('.left-sidebar__chat-item.active');
        if (activeItem) {
            activeItem.classList.remove('active');
        }
    }

    /**
     * Load the list of sessions from SQLite databases and render them in the left sidebar.
     */
    async loadSessionsList() {
        try {
            const sessions = await IPC.listSessions();
            this.sessions = sessions;
            
            // Group sessions by project/workspace
            const projectMap = new Map();
            const standaloneChats = [];
            
            sessions.forEach(session => {
                if (session.isProject) {
                    let proj = projectMap.get(session.workspace);
                    if (!proj) {
                        proj = {
                            id: session.workspace,
                            name: session.projectName,
                            chats: []
                        };
                        projectMap.set(session.workspace, proj);
                    }
                    proj.chats.push({
                        id: session.id,
                        title: session.title
                    });
                } else {
                    standaloneChats.push({
                        id: session.id,
                        title: session.title
                    });
                }
            });
            
            const projects = Array.from(projectMap.values());
            
            // Update left sidebar projects if available
            if (window.leftSidebarController) {
                window.leftSidebarController.mockData.projects = projects;
                window.leftSidebarController.renderProjects();
            }
            
            // Find sidebar chats container
            const chatsContent = document.querySelector('[data-section-content="chats"]');
            if (!chatsContent) return;
            
            chatsContent.innerHTML = '';
            
            if (standaloneChats.length === 0) {
                chatsContent.innerHTML = '<div class="left-sidebar__no-chats" style="padding: 12px 16px; font-size: 12px; color: #777777;">No recent chats</div>';
                return;
            }
            
            standaloneChats.forEach(chat => {
                const chatBtn = document.createElement('button');
                chatBtn.className = 'left-sidebar__chat-item';
                if (chat.id === this.activeSessionId) {
                    chatBtn.classList.add('active');
                }
                chatBtn.setAttribute('data-chat-id', chat.id);
                chatBtn.setAttribute('title', chat.title);
                
                const chatText = document.createElement('span');
                chatText.className = 'left-sidebar__chat-text';
                chatText.textContent = chat.title;
                
                // Hey friend! We create a delete button for this individual standalone chat session.
                // It utilizes the delete.svg icon and triggers the deleteSession API when clicked.
                const deleteBtn = document.createElement('span');
                deleteBtn.className = 'left-sidebar__chat-delete';
                deleteBtn.setAttribute('role', 'button');
                deleteBtn.setAttribute('aria-label', `Delete chat: ${chat.title}`);
                
                const deleteImg = document.createElement('img');
                deleteImg.src = './assets/icons/sidebar/delete.svg';
                deleteImg.alt = 'Delete Chat';
                deleteImg.className = 'left-sidebar__chat-delete-icon';
                deleteBtn.appendChild(deleteImg);
                
                chatBtn.appendChild(chatText);
                chatBtn.appendChild(deleteBtn);
                
                chatBtn.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.selectSession(chat.id);
                });
                
                // Hey friend! We prevent click event propagation so that selecting/opening the session isn't triggered
                // when the user is trying to delete the session.
                deleteBtn.addEventListener('click', (e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    this.deleteSession(chat.id);
                });
                
                chatsContent.appendChild(chatBtn);
            });
        } catch (error) {
            console.error('Failed to load sessions list:', error);
        }
    }

    /**
     * Select and load history for a specific chat session.
     * @param {string} sessionId - The session ID to open
     */
    async selectSession(sessionId) {
        if (this.activeSessionId === sessionId) return;
        
        this.activeSessionId = sessionId;
        this.resetStreamingState();
        
        // Update current project directory depending on if selected session is project-bound
        const session = this.sessions?.find(s => s.id === sessionId);
        if (session && session.isProject) {
            this.currentProjectDir = session.workspace;
        } else {
            this.currentProjectDir = null;
        }
        
        // Clear diagnostics bar if visible
        if (window.inputPanelController) {
            window.inputPanelController.clearDiagnostics();
            // Reset context usage display back to default placeholder values when changing the active chat session
            window.inputPanelController.updateContextUsage(0, 0, 0);
        }
        
        // Highlight active sidebar item
        document.querySelectorAll('.left-sidebar__chat-item').forEach(item => {
            if (item.getAttribute('data-chat-id') === sessionId) {
                item.classList.add('active');
            } else {
                item.classList.remove('active');
            }
        });
        
        // Hide empty state logo
        if (window.emptyStateController) {
            window.emptyStateController.hideEmptyState();
        }
        
        // Clear current screen messages
        if (window.userMessageController) {
            window.userMessageController.clearMessages();
        }
        if (window.assistantMessageController) {
            window.assistantMessageController.clearMessages();
        }
        
        try {
            // Load messages from SQLite turns database
            const history = await IPC.getSessionHistory(sessionId);
            
            if (history.length === 0) {
                // Hey friend! Since the history is empty, we start a new chat, but preserve
                // the current project directory configuration so the user stays in the project context.
                this.startNewChat(this.currentProjectDir);
                return;
            }
            
            // Extract the first user message for title
            let title = 'Chat Session';
            
            // 1. Build a map of tool results keyed by call_id
            const toolResultsMap = new Map();
            history.forEach(msg => {
                if (msg.role === 'Tool') {
                    msg.content.forEach(block => {
                        if (block.ToolResult) {
                            toolResultsMap.set(block.ToolResult.call_id, block.ToolResult);
                        }
                    });
                }
            });
            
            // 2. Render user and assistant turns
            history.forEach(msg => {
                if (msg.role === 'User') {
                    // Extract text
                    let text = '';
                    msg.content.forEach(block => {
                        if (typeof block === 'string') text += block;
                        else if (block.Text) text += block.Text;
                    });
                    
                    if (window.userMessageController) {
                        window.userMessageController.addMessage(text);
                    }
                    if (title === 'Chat Session') {
                        title = text.replace('\n', ' ').trim();
                        if (title.len > 40) title = title.substring(0, 40) + '...';
                    }
                } else if (msg.role === 'Assistant') {
                    // Render assistant message block with nested thinking / tool calls
                    this.renderHistoricalAssistantMessage(msg, toolResultsMap);
                }
            });
            
            // Update session title
            const titleEl = document.getElementById('session-title');
            if (titleEl) {
                titleEl.textContent = title;
            }
            
        } catch (error) {
            console.error('Failed to load session history:', error);
            showError('Failed to load chat history.');
        }
    }

    /**
     * Open a native folder picker and register the folder as the current project directory.
     * @returns {Promise<string|null>} - Selected path or null if cancelled
     */
    async openProject() {
        try {
            const projectPath = await IPC.openProjectFolder();
            if (projectPath) {
                // Hey friend! We pass the project path to startNewChat so the newly initialized
                // session is correctly bound as a project-specific chat.
                this.startNewChat(projectPath);
                
                // Show a toast or success notification
                showSuccess(`Opened project: ${projectPath.split(/[/\\]/).pop() || projectPath}`);
                
                // Refresh sessions list so that if there are sessions for this project, they are displayed
                await this.loadSessionsList();
                
                return projectPath;
            }
            return null;
        } catch (error) {
            console.error('Failed to open project:', error);
            showError(`Failed to open project: ${error.message || error}`);
            throw error;
        }
    }

    /**
     * Delete a specific chat session.
     *
     * Hey friend! This method triggers the deleteSession IPC command. If the deleted session
     * was the currently active one, we start a new empty chat. Finally, we reload the sessions list
     * to refresh the sidebar.
     *
     * @param {string} sessionId - The session ID to delete
     */
    async deleteSession(sessionId) {
        try {
            // Hey friend! The backend now shows a native confirmation dialog and returns true/false.
            const confirmed = await IPC.deleteSession(sessionId);
            if (confirmed) {
                showSuccess('Chat session deleted.');
                if (this.activeSessionId === sessionId) {
                    this.startNewChat();
                }
                await this.loadSessionsList();
            }
        } catch (error) {
            console.error('Failed to delete session:', error);
            showError('Failed to delete chat session.');
        }
    }

    /**
     * Delete a project and all its nested chat sessions.
     *
     * Hey friend! This method triggers the deleteProject IPC command. If the active session's workspace
     * belongs to the deleted project, we start a new empty chat. Finally, we reload the sessions list.
     *
     * @param {string} projectPath - The project path to delete
     */
    async deleteProject(projectPath) {
        try {
            // Hey friend! The backend shows a native confirmation dialog and returns true/false.
            const confirmed = await IPC.deleteProject(projectPath);
            if (confirmed) {
                showSuccess('Project deleted.');
                
                // Check if active session was in this project
                const activeSession = this.sessions?.find(s => s.id === this.activeSessionId);
                if (activeSession && activeSession.workspace === projectPath) {
                    this.startNewChat();
                }
                
                await this.loadSessionsList();
            }
        } catch (error) {
            console.error('Failed to delete project:', error);
            showError('Failed to delete project.');
        }
    }

    /**
     * Helper to render a historical assistant message including tool calls and results
     */
    /**
     * Helper to render a historical assistant message including tool calls and results
     * in the sequential order they were executed.
     */
    renderHistoricalAssistantMessage(msg, toolResultsMap) {
        if (!window.assistantMessageController) return;
        
        // Create an empty assistant message wrapper (passing null for the content)
        // so that we can append text, thinking, and tool blocks in their exact execution order.
        const msgEl = window.assistantMessageController.createMessage(null, "Just now");
        if (!msgEl) return;
        
        window.assistantMessageController.messagesContainer.appendChild(msgEl);
        const separator = msgEl.querySelector('.assistant-message__separator');
        
        // Loop through all blocks in sequence to build the sequential flow
        msg.content.forEach(block => {
            let blockText = null;
            if (typeof block === 'string') {
                blockText = block;
            } else if (block && block.Text) {
                blockText = block.Text;
            }
            
            if (blockText !== null) {
                // Render text block sequentially
                const contentDiv = document.createElement('div');
                contentDiv.className = 'assistant-message__content markdown-content';
                contentDiv.rawMarkdown = blockText;
                
                IPC.renderMarkdown(blockText)
                    .then(html => {
                        contentDiv.innerHTML = html;
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
                        // Apply syntax highlighting to code blocks in the loaded historical message block
                        if (window.highlightCodeBlocks) {
                            window.highlightCodeBlocks(contentDiv);
                        }
                    })
                    .catch(err => {
                        console.error("Failed to render historical markdown:", err);
                        contentDiv.textContent = blockText;
                    });
                
                msgEl.insertBefore(contentDiv, separator);
            } else if (block && block.Reasoning) {
                // Add a collapsed thinking card sequentially
                const thinkingCard = document.createElement('div');
                thinkingCard.className = 'assistant-message__thinking collapsed';
                thinkingCard.innerHTML = `
                    <div class="assistant-message__thinking-header">
                        <img class="assistant-message__thinking-icon" src="./assets/icons/sidebar/new-chat.svg" style="filter: invert(1); width:14px; height:14px;">
                        <span>Thinking Process</span>
                    </div>
                    <div class="assistant-message__thinking-content">${block.Reasoning.signature || block.Reasoning.thinking || ''}</div>
                `;
                
                // Toggle collapse on click
                thinkingCard.querySelector('.assistant-message__thinking-header').addEventListener('click', () => {
                    thinkingCard.classList.toggle('collapsed');
                });
                
                msgEl.insertBefore(thinkingCard, separator);
            } else if (block && block.ToolCall) {
                // Add tool card sequentially
                const call = block.ToolCall;
                const callId = call.id;
                const result = toolResultsMap.get(callId);
                
                let statusClass = 'assistant-message__tool-status--completed';
                let statusText = 'Completed';
                let resultText = 'No result returned.';
                
                if (result) {
                    if (result.is_error) {
                        statusClass = 'assistant-message__tool-status--failed';
                        statusText = 'Failed';
                    }
                    if (result.content) {
                        if (result.content.Text) resultText = result.content.Text;
                        else if (result.content.Json) resultText = JSON.stringify(result.content.Json, null, 2);
                        else if (typeof result.content === 'string') resultText = result.content;
                    }
                }
                
                const toolCard = document.createElement('div');
                toolCard.className = 'assistant-message__tool-card collapsed';
                
                let formattedArgs = '';
                try {
                    formattedArgs = JSON.stringify(call.arguments, null, 2);
                } catch (e) {
                    formattedArgs = JSON.stringify(call.arguments);
                }
                
                toolCard.innerHTML = `
                    <div class="assistant-message__tool-header">
                        <div class="assistant-message__tool-title-wrapper">
                            <span class="assistant-message__tool-icon">
                                <img src="./assets/icons/sidebar/plugins.svg" style="filter: invert(1); width:16px; height:16px;">
                            </span>
                            <span class="assistant-message__tool-name">${call.name}</span>
                        </div>
                        <span class="assistant-message__tool-status ${statusClass}">${statusText}</span>
                    </div>
                    <div class="assistant-message__tool-details">
                        <div class="assistant-message__tool-section">
                            <div class="assistant-message__tool-section-title">Arguments</div>
                            <pre class="assistant-message__tool-code">${formattedArgs}</pre>
                        </div>
                        <div class="assistant-message__tool-section">
                            <div class="assistant-message__tool-section-title">Result</div>
                            <pre class="assistant-message__tool-code">${resultText}</pre>
                        </div>
                    </div>
                `;
                
                // Toggle collapse on click
                toolCard.querySelector('.assistant-message__tool-header').addEventListener('click', () => {
                    toolCard.classList.toggle('collapsed');
                });
                
                msgEl.insertBefore(toolCard, separator);
            }
        });
        
        window.assistantMessageController.scrollToBottom();
    }

    /**
     * Send a user message to the active or new session.
     * @param {string} text - Message text to send
     */
    async sendUserMessage(text) {
        if (!text || !text.trim()) return;
        
        // Hide empty state if visible
        if (window.emptyStateController) {
            window.emptyStateController.hideEmptyState();
        }
        
        // Generate a new session ID if starting a new chat
        if (!this.activeSessionId) {
            this.activeSessionId = Date.now().toString(16) + Math.random().toString(16).substring(2, 6);
            
            // Set header title
            const titleEl = document.getElementById('session-title');
            if (titleEl) {
                titleEl.textContent = text.replace('\n', ' ').trim().substring(0, 40) + '...';
            }
        }
        
        this.resetStreamingState();
        
        // Render user message on screen
        if (window.userMessageController) {
            window.userMessageController.addMessage(text);
        }
        
        try {
            // Show typing indicator while waiting for the background response
            this.showTypingIndicator();
            
            // Invoke background send_message command
            await IPC.sendMessage(this.activeSessionId, text, this.currentProjectDir);
        } catch (error) {
            console.error('Failed to send message:', error);
            showError(error.toString());
            
            // If failed to launch, clean up state
            this.resetStreamingState();
        }
    }

    /**
     * Handle streaming event from the Rust SessionRunner
     * @param {Object} event - The deserialized SessionEvent enum
     */
    handleSessionEvent(event) {
        // Rust serialized enum format checks
        if (event.SessionStarted) {
            // Session starts
            this.loadSessionsList();
        } 
        else if (event.TextDelta) {
            this.ensureAssistantMessageCreated();
            this.ensureAssistantContentElCreated();
            const text = event.TextDelta.text;
            
            // Capture a reference to the active element to prevent race conditions during async markdown rendering
            const activeEl = this.currentAssistantContentEl;
            activeEl.rawMarkdown = (activeEl.rawMarkdown || "") + text;
            
            // Invoke the backend markdown parser to generate updated HTML
            IPC.renderMarkdown(activeEl.rawMarkdown)
                .then(html => {
                    activeEl.innerHTML = html;
                    // Run KaTeX equations auto-typeset over the message element
                    if (window.renderMathInElement) {
                        window.renderMathInElement(activeEl, {
                            delimiters: [
                                {left: '$$', right: '$$', display: true},
                                {left: '$', right: '$', display: false},
                                {left: '\\(', right: '\\)', display: false},
                                {left: '\\[', right: '\\]', display: true}
                            ],
                            throwOnError: false
                        });
                    }
                    // Apply syntax highlighting as the content streams in
                    if (window.highlightCodeBlocks) {
                        window.highlightCodeBlocks(activeEl);
                    }
                })
                .catch(err => {
                    console.error("Failed to render markdown delta:", err);
                    activeEl.textContent += text;
                });
                
            window.assistantMessageController.scrollToBottom();
        } 
        else if (event.ThinkingDelta) {
            this.ensureAssistantMessageCreated();
            this.ensureThinkingBlockCreated();
            const text = event.ThinkingDelta.text;
            this.currentThinkingContentEl.textContent += text;
            window.assistantMessageController.scrollToBottom();
        } 
        else if (event.ToolCallStart) {
            this.ensureAssistantMessageCreated();
            const { call_id, name } = event.ToolCallStart;
            this.createToolCallCard(call_id, name);
        } 
        else if (event.ToolCallArgsReady) {
            const { call_id, args_json } = event.ToolCallArgsReady;
            this.updateToolCallArgs(call_id, args_json);
        } 
        else if (event.ToolProgress) {
            // We can log progress
        } 
        else if (event.ToolCallResult) {
            const { call_id, is_error, content_json } = event.ToolCallResult;
            this.completeToolCallCard(call_id, is_error, content_json);
        } 
        else if (event.PermissionDenied) {
            // Tool call was denied globally or locally
            showError(`Permission Denied: ${event.PermissionDenied.reason}`);
        } 
        else if (event.ApprovalRequired) {
            this.ensureAssistantMessageCreated();
            const { id, tool, path, reason, args_json } = event.ApprovalRequired;
            this.createPermissionPromptCard(id, tool, path, reason, args_json);
        } 
        else if (event.ApprovalGranted) {
            const { id } = event.ApprovalGranted;
            this.resolvePermissionPrompt(id, true);
        }
        else if (event.AskQuestion) {
            this.ensureAssistantMessageCreated();
            const { id, question, options } = event.AskQuestion;
            this.createAskPromptCard(id, question, options);
        } 
        else if (event.PreTurnReady) {
            const { turn_index, message_count, tool_count, estimated_tokens } = event.PreTurnReady;
            if (window.inputPanelController) {
                window.inputPanelController.showDiagnosticsReady(turn_index, message_count, tool_count, estimated_tokens);
            }
        } 
        else if (event.PreTurnFailed) {
            const { turn_index, step, reason } = event.PreTurnFailed;
            if (window.inputPanelController) {
                window.inputPanelController.showDiagnosticsFailed(turn_index, step, reason);
            }
        }
        else if (event.ContextUsageUpdated) {
            // Retrieve current and total token counts, plus current usage ratio from backend event
            const { current_context_tokens, context_window, utilization } = event.ContextUsageUpdated;
            if (window.inputPanelController) {
                // Update the token count displays and color states dynamically
                window.inputPanelController.updateContextUsage(current_context_tokens, context_window, utilization);
            }
        }
        else if (event.Done) {
            // Turn completed successfully!
            // Collapse thinking block if finished
            if (this.currentThinkingEl) {
                this.currentThinkingEl.classList.add('collapsed');
            }
            // Highlight the code blocks in the finished streaming message
            if (this.currentAssistantContentEl && window.highlightCodeBlocks) {
                window.highlightCodeBlocks(this.currentAssistantContentEl);
            }
            this.resetStreamingState();
            this.loadSessionsList();
        } 
        else if (event.Error) {
            showError(`Session Error: ${event.Error.message}`);
            this.resetStreamingState();
        }
    }

    /**
     * Lazily initialize assistant message DOM elements for streaming
     */
    /**
     * Lazily initialize assistant message DOM elements for streaming.
     * We pass null to createMessage to start with a clean container, allowing sequential streams.
     */
    ensureAssistantMessageCreated() {
        if (!this.currentAssistantMsgEl) {
            this.hideTypingIndicator(); // Hide typing indicator when response starts streaming
            
            if (!window.assistantMessageController) return;
            
            // Create a message container without default content
            const msgEl = window.assistantMessageController.createMessage(null, "Just now");
            window.assistantMessageController.messagesContainer.appendChild(msgEl);
            
            this.currentAssistantMsgEl = msgEl;
            this.currentAssistantContentEl = null;
            
            window.assistantMessageController.scrollToBottom();
        }
    }

    /**
     * Ensure we have an active text content element ready for streaming text.
     * If transitioning from a thinking block, we collapse it and create a new text block.
     */
    ensureAssistantContentElCreated() {
        // If we were thinking, collapse that thinking block as we are now transitioning to text response.
        if (this.currentThinkingEl) {
            this.currentThinkingEl.classList.add('collapsed');
            this.currentThinkingEl = null;
        }

        if (!this.currentAssistantContentEl) {
            const contentDiv = document.createElement('div');
            contentDiv.className = 'assistant-message__content markdown-content';
            contentDiv.textContent = "";
            contentDiv.rawMarkdown = "";
            
            const separator = this.currentAssistantMsgEl.querySelector('.assistant-message__separator');
            this.currentAssistantMsgEl.insertBefore(contentDiv, separator);
            
            this.currentAssistantContentEl = contentDiv;
        }
    }

    /**
     * Lazily initialize inline thinking box for streaming thinking process.
     * If transitioning from text, we clear current text block reference to force a new block next time.
     */
    ensureThinkingBlockCreated() {
        // Transitioning to thinking, so any subsequent text will start in a new text block.
        this.currentAssistantContentEl = null;

        if (!this.currentThinkingEl) {
            const thinkingCard = document.createElement('div');
            thinkingCard.className = 'assistant-message__thinking';
            thinkingCard.innerHTML = `
                <div class="assistant-message__thinking-header">
                    <img class="assistant-message__thinking-icon" src="./assets/icons/sidebar/new-chat.svg" style="filter: invert(0.6); width:14px; height:14px;">
                    <span>Thinking Process</span>
                </div>
                <div class="assistant-message__thinking-content"></div>
            `;
            
            // Toggle collapse on click
            thinkingCard.querySelector('.assistant-message__thinking-header').addEventListener('click', () => {
                thinkingCard.classList.toggle('collapsed');
            });
            
            // Insert before separator and actions row
            const separator = this.currentAssistantMsgEl.querySelector('.assistant-message__separator');
            this.currentAssistantMsgEl.insertBefore(thinkingCard, separator);
            
            this.currentThinkingEl = thinkingCard;
            this.currentThinkingContentEl = thinkingCard.querySelector('.assistant-message__thinking-content');
        }
    }

    /**
     * Create inline tool card
     */
    /**
     * Create inline tool card
     */
    createToolCallCard(callId, name) {
        // If we were thinking, collapse that thinking block.
        if (this.currentThinkingEl) {
            this.currentThinkingEl.classList.add('collapsed');
            this.currentThinkingEl = null;
        }
        // Force new text/thinking blocks after this tool execution card.
        this.currentAssistantContentEl = null;

        const toolCard = document.createElement('div');
        toolCard.className = 'assistant-message__tool-card';
        toolCard.id = `tool-${callId}`;
        toolCard.innerHTML = `
            <div class="assistant-message__tool-header">
                <div class="assistant-message__tool-title-wrapper">
                    <span class="assistant-message__tool-icon">
                        <img src="./assets/icons/sidebar/plugins.svg" style="filter: invert(1); width:16px; height:16px;">
                    </span>
                    <span class="assistant-message__tool-name">tool: ${name}</span>
                </div>
                <span class="assistant-message__tool-status assistant-message__tool-status--running">
                    <span class="tool-spinner"></span>Running
                </span>
            </div>
            <div class="assistant-message__tool-details">
                <div class="assistant-message__tool-section">
                    <div class="assistant-message__tool-section-title">Arguments</div>
                    <pre class="assistant-message__tool-code">Pending...</pre>
                </div>
            </div>
        `;
        
        // Toggle collapse
        toolCard.querySelector('.assistant-message__tool-header').addEventListener('click', () => {
            toolCard.classList.toggle('collapsed');
        });
        
        const separator = this.currentAssistantMsgEl.querySelector('.assistant-message__separator');
        this.currentAssistantMsgEl.insertBefore(toolCard, separator);
        
        this.activeToolCalls.set(callId, toolCard);
        window.assistantMessageController.scrollToBottom();
    }

    /**
     * Update tool arguments in inline tool card
     */
    updateToolCallArgs(callId, argsJson) {
        const toolCard = this.activeToolCalls.get(callId);
        if (toolCard) {
            const codeEl = toolCard.querySelector('.assistant-message__tool-code');
            if (codeEl) {
                try {
                    const parsed = JSON.parse(argsJson);
                    codeEl.textContent = JSON.stringify(parsed, null, 2);
                } catch (e) {
                    codeEl.textContent = argsJson;
                }
            }
        }
    }

    /**
     * Complete inline tool card execution, setting status and result
     */
    completeToolCallCard(callId, isError, contentJson) {
        const toolCard = this.activeToolCalls.get(callId);
        if (toolCard) {
            // Update status badge
            const statusEl = toolCard.querySelector('.assistant-message__tool-status');
            if (statusEl) {
                if (isError) {
                    statusEl.className = 'assistant-message__tool-status assistant-message__tool-status--failed';
                    statusEl.textContent = 'Failed';
                } else {
                    statusEl.className = 'assistant-message__tool-status assistant-message__tool-status--completed';
                    statusEl.textContent = 'Completed';
                }
            }
            
            // Append result section
            const detailsEl = toolCard.querySelector('.assistant-message__tool-details');
            if (detailsEl) {
                const resultSection = document.createElement('div');
                resultSection.className = 'assistant-message__tool-section';
                
                let cleanResult = contentJson;
                try {
                    const parsed = JSON.parse(contentJson);
                    cleanResult = JSON.stringify(parsed, null, 2);
                } catch (e) {}
                
                resultSection.innerHTML = `
                    <div class="assistant-message__tool-section-title">Result</div>
                    <pre class="assistant-message__tool-code">${cleanResult}</pre>
                `;
                detailsEl.appendChild(resultSection);
            }
            
            // Collapse automatically to keep the feed clean
            toolCard.classList.add('collapsed');
        }
    }

    /**
     * Create inline permission approval prompt card
     */
    /**
     * Create inline permission approval prompt card
     */
    createPermissionPromptCard(id, tool, path, reason, argsJson) {
        // Collapse active thinking block when prompt appears.
        if (this.currentThinkingEl) {
            this.currentThinkingEl.classList.add('collapsed');
            this.currentThinkingEl = null;
        }
        // Force new text/thinking blocks after this permission card.
        this.currentAssistantContentEl = null;

        const permCard = document.createElement('div');
        permCard.className = 'assistant-message__permission-card';
        permCard.id = `approval-${id}`;
        
        let pathInfo = '';
        if (path) {
            pathInfo = ` on <code style="background: rgba(0,0,0,0.3); padding: 2px 4px; border-radius: 3px;">${path}</code>`;
        }
        
        permCard.innerHTML = `
            <div class="assistant-message__permission-header">
                <img class="assistant-message__permission-warning-icon" src="./assets/icons/sidebar/settings.svg" style="filter: invert(0.8) sepia(1) saturate(5) hue-rotate(5deg); width:18px; height:18px;">
                <span class="assistant-message__permission-title">Permission Requested</span>
            </div>
            <div class="assistant-message__permission-body">
                <div class="assistant-message__permission-reason" style="margin-bottom: 12px; font-size:13px; line-height:18px;">
                    The model requests permission to execute <strong>${tool}</strong>${pathInfo}.<br>
                    <span style="color: #bbbbbb; font-style: italic;">Reason: ${reason}</span>
                </div>
                <div class="assistant-message__permission-actions">
                    <button class="btn-permission btn-permission--allow">Allow</button>
                    <button class="btn-permission btn-permission--deny">Deny</button>
                </div>
            </div>
        `;
        
        // Bind button actions
        permCard.querySelector('.btn-permission--allow').addEventListener('click', async () => {
            permCard.querySelectorAll('.btn-permission').forEach(b => b.disabled = true);
            try {
                await IPC.approveToolCall(this.activeSessionId, id);
            } catch (err) {
                showError(err.toString());
                permCard.querySelectorAll('.btn-permission').forEach(b => b.disabled = false);
            }
        });
        
        permCard.querySelector('.btn-permission--deny').addEventListener('click', async () => {
            permCard.querySelectorAll('.btn-permission').forEach(b => b.disabled = true);
            try {
                await IPC.denyToolCall(this.activeSessionId, id);
                this.resolvePermissionPrompt(id, false);
            } catch (err) {
                showError(err.toString());
                permCard.querySelectorAll('.btn-permission').forEach(b => b.disabled = false);
            }
        });
        
        const separator = this.currentAssistantMsgEl.querySelector('.assistant-message__separator');
        this.currentAssistantMsgEl.insertBefore(permCard, separator);
        
        this.activePermissionPrompts.set(id, permCard);
        window.assistantMessageController.scrollToBottom();
    }

    /**
     * Resolve/update an inline permission prompt card once approved or denied
     */
    resolvePermissionPrompt(id, approved) {
        const permCard = this.activePermissionPrompts.get(id);
        if (permCard) {
            const actionsEl = permCard.querySelector('.assistant-message__permission-actions');
            if (actionsEl) {
                if (approved) {
                    actionsEl.innerHTML = `
                        <span class="assistant-message__permission-status assistant-message__permission-status--approved">
                            ✓ Approved
                        </span>
                    `;
                } else {
                    actionsEl.innerHTML = `
                        <span class="assistant-message__permission-status assistant-message__permission-status--denied">
                            ✗ Denied
                        </span>
                    `;
                }
            }
        }
    }

    /**
     * Create inline ask question / MCQ card
     */
    createAskPromptCard(id, question, options) {
        // Collapse active thinking block when prompt appears.
        if (this.currentThinkingEl) {
            this.currentThinkingEl.classList.add('collapsed');
            this.currentThinkingEl = null;
        }
        // Force new text/thinking blocks after this card.
        this.currentAssistantContentEl = null;

        const askCard = document.createElement('div');
        askCard.className = 'assistant-message__ask-card';
        askCard.id = `ask-${id}`;
        
        let optionsHtml = options.map((opt, index) => {
            return `<button class="btn-ask-option" data-option="${opt}">${opt}</button>`;
        }).join('');

        askCard.innerHTML = `
            <div class="assistant-message__ask-header">
                <img class="assistant-message__ask-icon" src="./assets/icons/sidebar/new-chat.svg" style="filter: invert(0.6) sepia(1) saturate(5) hue-rotate(180deg); width:18px; height:18px;">
                <span class="assistant-message__ask-title">Question Prompt</span>
            </div>
            <div class="assistant-message__ask-body">
                <div class="assistant-message__ask-question">${question}</div>
                <div class="assistant-message__ask-options">
                    ${optionsHtml}
                </div>
                <div class="assistant-message__ask-custom">
                    <input type="text" class="input-ask-custom" placeholder="Or type a custom answer..." />
                    <button class="btn-ask-submit">Submit</button>
                </div>
            </div>
        `;

        // Bind options buttons
        askCard.querySelectorAll('.btn-ask-option').forEach(btn => {
            btn.addEventListener('click', async () => {
                const answer = btn.getAttribute('data-option');
                this.submitAskAnswer(id, answer, askCard);
            });
        });

        // Bind custom submit button
        const submitBtn = askCard.querySelector('.btn-ask-submit');
        const inputEl = askCard.querySelector('.input-ask-custom');
        
        const doCustomSubmit = () => {
            const answer = inputEl.value.trim();
            if (answer) {
                this.submitAskAnswer(id, answer, askCard);
            }
        };

        submitBtn.addEventListener('click', doCustomSubmit);
        inputEl.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                doCustomSubmit();
            }
        });

        const separator = this.currentAssistantMsgEl.querySelector('.assistant-message__separator');
        this.currentAssistantMsgEl.insertBefore(askCard, separator);
        
        this.activeAskPrompts.set(id, askCard);
        window.assistantMessageController.scrollToBottom();
    }

    /**
     * Submit user's answer to Tauri backend and resolve UI card
     */
    async submitAskAnswer(id, answer, askCard) {
        // Disable all inputs
        askCard.querySelectorAll('.btn-ask-option, .btn-ask-submit, .input-ask-custom').forEach(el => {
            el.disabled = true;
        });

        try {
            await IPC.answerAsk(this.activeSessionId, id, answer);
            this.resolveAskPrompt(id, answer);
        } catch (err) {
            showError(err.toString());
            // Re-enable inputs on error
            askCard.querySelectorAll('.btn-ask-option, .btn-ask-submit, .input-ask-custom').forEach(el => {
                el.disabled = false;
            });
        }
    }

    /**
     * Resolve ask prompt card in the UI
     */
    resolveAskPrompt(id, answer) {
        const askCard = this.activeAskPrompts.get(id);
        if (askCard) {
            const bodyEl = askCard.querySelector('.assistant-message__ask-body');
            if (bodyEl) {
                bodyEl.innerHTML = `
                    <div class="assistant-message__ask-status" style="color: #4caf50; font-weight: 600; font-size: 13.5px; display: flex; align-items: center; gap: 8px;">
                        ✓ Answered: <strong>${answer}</strong>
                    </div>
                `;
            }
        }
    }
}

// Create and export singleton instance
const sessionManager = new SessionManager();

// Auto-initialize once DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        sessionManager.init();
        window.sessionManager = sessionManager;
    });
} else {
    sessionManager.init();
    window.sessionManager = sessionManager;
}

export default sessionManager;
