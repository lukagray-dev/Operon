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

// Helper to escape HTML characters in templates to prevent parser glitches & XSS
function escapeHtml(raw) {
    if (raw === null || raw === undefined) return '';
    return String(raw)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#039;');
}

// Helper to escape HTML attribute values safely
function escapeAttribute(raw) {
    return escapeHtml(raw);
}

/**
 * Parses tool arguments from their raw string representation.
 *
 * During live streaming, the Rust runner emits events where the arguments are not formatted
 * as a single JSON object. Instead, the backend formats it as:
 *   {"path":"C:\\..."}
 *   __body__:
 *   <raw content lines>
 * OR simply:
 *   __body__:
 *   <raw content lines>
 *
 * But when loading the history from the SQLite database, it is stored as a valid, fully formed
 * JSON string:
 *   {"path":"C:\\...", "__body__":"<escaped content>"}
 *
 * This helper function attempts to handle both cases transparently, so the live UI and the
 * reloaded history UI look identical.
 *
 * @param {string} argsJson - The raw arguments string to parse.
 * @returns {Object|null} A parsed object with path and __body__ properties, or null if parsing fails.
 */
function parseArgsJson(argsJson) {
    if (!argsJson) return null;
    
    // Attempt 1: Standard JSON parsing. This succeeds for historical messages or valid JSON payloads.
    try {
        return JSON.parse(argsJson);
    } catch (e) {
        // Attempt 2: Fall back to parsing the custom multi-line text format sent during live stream.
    }
    
    const lines = argsJson.split('\n');
    let path = '';
    let bodyStartIndex = -1;
    
    // Check if the first line is a JSON block containing the metadata (e.g. {"path": "..."})
    if (lines.length > 0 && lines[0].trim().startsWith('{')) {
        try {
            const parsedFirstLine = JSON.parse(lines[0]);
            path = parsedFirstLine.path || parsedFirstLine.paths || '';
        } catch (e) {
            // First line was not valid JSON after all, or didn't contain a path attribute.
        }
    }
    
    // Look for the "__body__:" line delimiter that separates metadata from the raw content lines.
    for (let i = 0; i < lines.length; i++) {
        if (lines[i].trim() === '__body__:') {
            bodyStartIndex = i + 1;
            break;
        }
    }
    
    let body = '';
    if (bodyStartIndex !== -1) {
        // Extract everything following the "__body__:" line and join them back with newlines.
        body = lines.slice(bodyStartIndex).join('\n');
    } else {
        // Fallback: If no "__body__:" delimiter was found, and the first line was not JSON,
        // treat the entire string as the raw body.
        if (!lines[0].trim().startsWith('{')) {
            body = argsJson;
        }
    }
    
    return {
        path: path,
        __body__: body
    };
}

/**
 * Detects the programming language for syntax highlighting based on the file extension.
 * Maps common file extensions to Highlight.js language identifiers.
 *
 * @param {string} path - The file path (e.g., 'src/main.rs').
 * @returns {string} The Highlight.js language identifier (e.g., 'rust'), or empty string if unknown.
 */
function detectLanguage(path) {
    if (!path) return '';
    const parts = path.split(/[/\\]/);
    const filename = parts[parts.length - 1];
    const extParts = filename.split('.');
    if (extParts.length <= 1) return '';
    const ext = extParts.pop().toLowerCase();
    
    // Map of common file extensions to Highlight.js language names
    const langMap = {
        'js': 'javascript',
        'mjs': 'javascript',
        'cjs': 'javascript',
        'ts': 'typescript',
        'tsx': 'typescript',
        'py': 'python',
        'rs': 'rust',
        'sh': 'bash',
        'bash': 'bash',
        'json': 'json',
        'css': 'css',
        'html': 'html',
        'htm': 'html',
        'cpp': 'cpp',
        'cc': 'cpp',
        'cxx': 'cpp',
        'c': 'c',
        'h': 'cpp',
        'hpp': 'cpp',
        'cs': 'csharp',
        'go': 'go',
        'rb': 'ruby',
        'md': 'markdown',
        'toml': 'ini',
        'yaml': 'yaml',
        'yml': 'yaml',
        'xml': 'xml',
        'sql': 'sql',
        'bat': 'cmd',
        'cmd': 'cmd',
        'ps1': 'powershell'
    };
    return langMap[ext] || '';
}

/**
 * Extracts the raw name of a tool from card header text.
 * The card header might look like "tool: write path='...'" (live cards)
 * or just "write" (historical cards loaded from SQLite).
 *
 * @param {string} text - The raw text content of the tool name label.
 * @returns {string} The extracted tool name, e.g., 'write', 'append', 'edit'.
 */
function extractToolName(text) {
    if (!text) return '';
    const clean = text.trim();
    // Live cards have a "tool:" prefix (e.g. "tool: write"). We strip this prefix
    // and grab the first word after it as the actual tool name.
    if (clean.startsWith('tool:')) {
        const parts = clean.slice(5).trim().split(/\s+/);
        return parts[0];
    }
    // Historical cards just have the raw tool name (e.g. "write"), so we split
    // by spaces just in case and grab the first token.
    return clean.split(/\s+/)[0];
}

/**
 * Extracts a file/directory path from an raw XML-like tool attribute string (e.g. 'path="D:\Project\main.js"').
 * This is used to extract paths while the tool call is still in progress / streaming.
 *
 * @param {string} attrs - The raw attributes string from the tool call element.
 * @returns {string} The parsed path value, or an empty string if not found.
 */
function extractPathFromAttrs(attrs) {
    if (!attrs) return '';
    // Look for path="...", paths="...", or dir="..." (both double and single quotes)
    const match = attrs.match(/(?:path|paths|dir)\s*=\s*"([^"]+)"/) || attrs.match(/(?:path|paths|dir)\s*=\s*'([^']+)'/);
    return match ? match[1] : '';
}

/**
 * Generates a human-friendly tool execution card title and tooltip based on the active state (running vs completed).
 * Supports both single-file and multi-file paths (like the "read" tool which lists paths separated by newlines).
 *
 * @param {string} name - The tool name (e.g., 'edit', 'write', 'append', 'ls', 'bash').
 * @param {Object} argsObj - The parsed JSON arguments (contains path/paths/dir/etc).
 * @param {boolean} isCompleted - Whether the tool call execution has completed.
 * @returns {Object} An object containing `{ title, tooltip }`.
 */
function getToolHeaderTitle(name, argsObj, isCompleted) {
    const path = argsObj?.path || argsObj?.paths || argsObj?.dir || '';
    let displayName = '';
    let tooltip = '';
    
    if (path) {
        let pathEntries = [];
        // The read tool supports multiple path entries separated by newlines
        if (path.includes('\n')) {
            pathEntries = path.split('\n').map(p => p.trim()).filter(p => p.length > 0);
        } else {
            const trimmed = path.trim();
            if (trimmed) {
                pathEntries = [trimmed];
            }
        }
        
        if (pathEntries.length > 0) {
            const fileNames = pathEntries.map(p => {
                // Strip optional line ranges like :40-90 or :50-
                const cleanPath = p.replace(/:\d*-\d*$/, '');
                const parts = cleanPath.split(/[/\\]/);
                return parts[parts.length - 1] || cleanPath;
            });
            // Join multiple file names with a comma to list them in the header
            displayName = fileNames.join(', ');
            
            // Join full paths with newlines to show each path clearly in the hover tooltip popup
            tooltip = pathEntries.map(p => p.replace(/:\d*-\d*$/, '')).join('\n');
        }
    }

    let title = '';
    switch (name) {
        case 'write':
            title = isCompleted ? `Wrote ${displayName || 'file'}` : `Writing ${displayName || 'file'}`;
            break;
        case 'append':
            title = isCompleted ? `Appended ${displayName || 'file'}` : `Appending ${displayName || 'file'}`;
            break;
        case 'edit':
            title = isCompleted ? `Edited ${displayName || 'file'}` : `Editing ${displayName || 'file'}`;
            break;
        case 'read':
            title = isCompleted ? `Read ${displayName || 'file'}` : `Reading ${displayName || 'file'}`;
            break;
        case 'delete':
            title = isCompleted ? `Deleted ${displayName || 'file'}` : `Deleting ${displayName || 'file'}`;
            break;
        case 'ls':
            title = isCompleted ? `Listed ${displayName || 'directory'}` : `Listing ${displayName || 'directory'}`;
            break;
        case 'grep':
            title = isCompleted ? `Searched ${displayName || 'directory'}` : `Searching ${displayName || 'directory'}`;
            break;
        case 'bash':
            title = isCompleted ? 'Executed command' : 'Executing command';
            break;
        case 'ask':
            title = isCompleted ? 'Asked question' : 'Asking question';
            break;
        case 'web_search':
            title = isCompleted ? 'Searched web' : 'Searching web';
            break;
        case 'web_fetch':
            title = isCompleted ? 'Fetched web page' : 'Fetching web page';
            break;
        case 'todo_create':
            title = isCompleted ? 'Created TODO' : 'Creating TODO';
            break;
        case 'todo_update':
            title = isCompleted ? 'Updated TODO' : 'Updating TODO';
            break;
        case 'todo_list':
            title = isCompleted ? 'Listed TODOs' : 'Listing TODOs';
            break;
        default:
            const formattedName = name.charAt(0).toUpperCase() + name.slice(1);
            title = isCompleted ? `Finished ${formattedName}` : `Running ${formattedName}`;
            break;
    }
    
    return { title, tooltip };
}


/**
 * Calculates line-based insertion and deletion statistics for our code modification tools.
 *
 * For "write" and "append", everything in the body content is treated as newly added lines,
 * and there are no deletions (0).
 * For "edit", the body content is a unified diff structure where lines starting with '+'
 * represent additions, and lines starting with '-' represent deletions.
 *
 * @param {string} name - The tool name.
 * @param {Object} argsObj - The parsed JSON arguments object of the tool call.
 * @returns {Object} An object containing { added, deleted } counts.
 */
function getToolDiffStats(name, argsObj) {
    let added = 0;
    let deleted = 0;
    if (!argsObj) return { added, deleted };

    if (name === 'write' || name === 'append') {
        // Grab the text payload. In the dispatcher, this is usually mapped to "__body__"
        // or sometimes "content". If neither is present, fallback to empty string.
        const body = argsObj.__body__ || argsObj.content || '';
        if (body) {
            // Count total lines. Even an empty string with newlines is split,
            // so we count the items in the split array.
            added = body.split('\n').length;
        }
    } else if (name === 'edit') {
        // In the edit tool, the body contains the diff hunk. We split it into lines
        // and count lines starting with '+' (additions) or '-' (deletions).
        const body = argsObj.__body__ || '';
        const lines = body.split('\n');
        for (let line of lines) {
            if (line.startsWith('+')) {
                added++;
            } else if (line.startsWith('-')) {
                deleted++;
            }
        }
    }
    return { added, deleted };
}

/**
 * Generates interactive, color-coded HTML representing the diff/code changes.
 *
 * This renders the code block with a green background for additions and red for deletions.
 * - For "write" and "append", all lines are wrapped in addition style (.diff-line--added).
 * - For "edit", lines starting with '+' are additions, '-' are deletions, ' ' are context,
 *   and '@@' are headers. The prefixes are stripped for clean presentation, matching Codex.
 *
 * @param {string} name - The tool name.
 * @param {Object} argsObj - The parsed JSON arguments object of the tool call.
 * @returns {string} The fully formed HTML string representing the diff.
 */
function renderToolDiffHTML(name, argsObj) {
    if (!argsObj) return '';
    let html = '';
    
    const path = argsObj.path || argsObj.paths || '';
 
    // Detect the programming language using the file extension helper
    const lang = detectLanguage(path);

    if (name === 'write' || name === 'append') {
        const body = argsObj.__body__ || argsObj.content || '';
        const lines = body.split('\n');
        html += `<div class="diff-lines-container">`;
        html += `<div class="diff-lines-wrapper">`;
        for (let line of lines) {
            // Apply syntax highlighting to the code line if hljs is present
            let highlighted = '';
            if (typeof hljs !== 'undefined' && lang) {
                try {
                    highlighted = hljs.highlight(line, { language: lang }).value;
                } catch (e) {
                    highlighted = escapeHtml(line);
                }
            } else {
                highlighted = escapeHtml(line);
            }
            // Mark every line as added, since write/append creates/adds content.
            html += `<div class="diff-line diff-line--added">${highlighted}</div>`;
        }
        html += `</div>`;
        html += `</div>`;
    } else if (name === 'edit') {
        const body = argsObj.__body__ || '';
        const lines = body.split('\n');
        html += `<div class="diff-lines-container">`;
        html += `<div class="diff-lines-wrapper">`;
        for (let line of lines) {
            // Check for hunk headers (lines starting with '@@')
            if (line.startsWith('@@')) {
                // Per user request, omit @@ hunk header lines entirely to clean up card presentation.
                continue;
            }
            
            let prefix = '';
            let content = line;
            let className = 'diff-line';
            
            // Classify each line's prefix (+ / - / space) to set background colors
            if (line.startsWith('+')) {
                prefix = '+';
                content = line.slice(1);
                className = 'diff-line diff-line--added';
            } else if (line.startsWith('-')) {
                prefix = '-';
                content = line.slice(1);
                className = 'diff-line diff-line--removed';
            } else if (line.startsWith(' ')) {
                prefix = ' ';
                content = line.slice(1);
                className = 'diff-line diff-line--context';
            } else if (line.trim().length > 0) {
                // Handle metadata lines (e.g. @@ EOF or fallback descriptors)
                className = 'diff-line diff-line--meta';
            } else {
                className = 'diff-line';
            }
            
            // Syntax highlight the code content (excluding unified diff prefixes so parser doesn't get confused)
            let highlighted = '';
            if (typeof hljs !== 'undefined' && lang && (prefix === '+' || prefix === '-' || prefix === ' ')) {
                try {
                    highlighted = hljs.highlight(content, { language: lang }).value;
                } catch (e) {
                    highlighted = escapeHtml(content);
                }
            } else {
                highlighted = escapeHtml(content);
            }
            
            html += `<div class="${className}">${highlighted}</div>`;
        }
        html += `</div>`;
        html += `</div>`;
    }
    return html;
}

/**
 * Updates the tool card header by injecting the diff stats (+12, -5) right beside the badge.
 * This is called for both live streaming and historical rendering of write, append, and edit.
 *
 * @param {HTMLElement} toolCard - The DOM element of the tool card.
 * @param {string} name - The tool name.
 * @param {Object} argsObj - The parsed JSON arguments object of the tool call.
 */
function updateToolCardDiffStats(toolCard, name, argsObj) {
    if (name !== 'write' && name !== 'append' && name !== 'edit') return;
    const { added, deleted } = getToolDiffStats(name, argsObj);
    if (added === 0 && deleted === 0) return;
    
    // Find the wrapper element where status badge and chevron reside.
    const statusWrapper = toolCard.querySelector('.assistant-message__tool-status-wrapper');
    if (statusWrapper) {
        // If we already added stats previously (e.g. during a delta update), remove the old element.
        const existingStats = statusWrapper.querySelector('.assistant-message__tool-diff-stats');
        if (existingStats) {
            existingStats.remove();
        }
        
        const statsEl = document.createElement('div');
        statsEl.className = 'assistant-message__tool-diff-stats';
        
        let statsHtml = '';
        if (added > 0) {
            statsHtml += `<span class="diff-stat-added">+${added}</span>`;
        }
        if (deleted > 0) {
            statsHtml += `<span class="diff-stat-deleted">-${deleted}</span>`;
        }
        statsEl.innerHTML = statsHtml;
        
        // Insert the stats block as the first child of statusWrapper, placing it
        // directly to the left of the checkmark/failed badge.
        statusWrapper.insertBefore(statsEl, statusWrapper.firstChild);
    }
}

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
            window.inputPanelController.setGeneratingState(false);
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
            window.inputPanelController.setGeneratingState(false);
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
            let currentAssistantMsgEl = null;
            history.forEach(msg => {
                if (msg.role === 'User') {
                    // Reset reference when transitioning to a new User message turn
                    currentAssistantMsgEl = null;
                    
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
                        if (title.length > 40) title = title.substring(0, 40) + '...';
                    }
                } else if (msg.role === 'Assistant') {
                    // Render assistant message block with nested thinking / tool calls.
                    // Hey friend! We group consecutive assistant message blocks into a single message element,
                    // which prevents redundant separator lines and feedback action rows under each individual block/tool call.
                    if (!currentAssistantMsgEl) {
                        currentAssistantMsgEl = window.assistantMessageController.createMessage(null, "Just now");
                        if (currentAssistantMsgEl) {
                            window.assistantMessageController.messagesContainer.appendChild(currentAssistantMsgEl);
                        }
                    }
                    if (currentAssistantMsgEl) {
                        this.renderHistoricalAssistantMessage(currentAssistantMsgEl, msg, toolResultsMap);
                    }
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
    /**
     * Helper to render a historical assistant message including tool calls and results
     * in the sequential order they were executed.
     * 
     * Hey friend! This method now takes the existing message wrapper element (msgEl)
     * as its first parameter and inserts the sequential flow of blocks (text, thinking, tool calls)
     * right before the separator element.
     */
    renderHistoricalAssistantMessage(msgEl, msg, toolResultsMap) {
        if (!msgEl) return;
        const separator = msgEl.querySelector('.assistant-message__separator');
        if (!separator) return;
        
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
                        <img class="assistant-message__thinking-icon" src="./assets/icons/main-content/messages/assistant/thinking.svg" style="filter: invert(0.6); width:14px; height:14px;">
                        <span>Thinking Process</span>
                    </div>
                    <div class="assistant-message__thinking-content"></div>
                `;
                thinkingCard.querySelector('.assistant-message__thinking-content').textContent = block.Reasoning.thinking || block.Reasoning.signature || '';
                
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
                let statusText = '<img class="assistant-message__tool-status-icon assistant-message__tool-status-icon--completed" src="./assets/icons/main-content/messages/assistant/circle-check.svg">';
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
                toolCard.dataset.toolName = call.name; // Store tool name for consistent access
                
                // Check if this tool is a code-modification/diff tool
                const isDiffTool = (call.name === 'write' || call.name === 'append' || call.name === 'edit');
                
                let detailsHtml = '';
                if (isDiffTool) {
                    // For code tools, render the arguments as a beautiful diff (added/deleted lines)
                    detailsHtml = renderToolDiffHTML(call.name, call.arguments);
                } else {
                    // For general tools, stick to the classic Arguments and Result code blocks
                    let formattedArgs = '';
                    try {
                        formattedArgs = JSON.stringify(call.arguments, null, 2);
                    } catch (e) {
                        formattedArgs = JSON.stringify(call.arguments);
                    }
                    
                    detailsHtml = `
                        <div class="assistant-message__tool-section">
                            <div class="assistant-message__tool-section-title">Arguments</div>
                            <pre class="assistant-message__tool-code">${escapeHtml(formattedArgs)}</pre>
                        </div>
                        <div class="assistant-message__tool-section">
                            <div class="assistant-message__tool-section-title">Result</div>
                            <pre class="assistant-message__tool-code">${escapeHtml(resultText)}</pre>
                        </div>
                    `;
                }

                // Generate a completed action header title (e.g. "Edited ipc.js") and set full path hover tooltip
                const { title: headerTitle, tooltip: pathVal } = getToolHeaderTitle(call.name, call.arguments, true);
                const tooltipAttr = pathVal ? `title="${escapeAttribute(pathVal)}"` : '';
                
                toolCard.innerHTML = `
                    <div class="assistant-message__tool-header">
                        <div class="assistant-message__tool-title-wrapper">
                            <span class="assistant-message__tool-icon">
                                <img src="./assets/icons/main-content/messages/assistant/tool.svg" style="filter: invert(0.7); width:14px; height:14px;">
                            </span>
                            <span class="assistant-message__tool-name" ${tooltipAttr}>${escapeHtml(headerTitle)}</span>
                        </div>
                        <div class="assistant-message__tool-status-wrapper" style="display: flex; align-items: center; gap: 8px;">
                            <span class="assistant-message__tool-status ${statusClass}">${statusText}</span>
                            <img class="assistant-message__tool-chevron" src="./assets/icons/sidebar/chevron-down.svg" style="filter: invert(0.6); width: 14px; height: 14px;">
                        </div>
                    </div>
                    <div class="assistant-message__tool-details">
                        ${detailsHtml}
                    </div>
                `;
                
                // If it is a diff tool, count and display the added/removed lines in the header badge
                if (isDiffTool) {
                    updateToolCardDiffStats(toolCard, call.name, call.arguments);
                }
                
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
            if (window.inputPanelController) {
                window.inputPanelController.setGeneratingState(true);
            }
            
            // Invoke background send_message command
            await IPC.sendMessage(this.activeSessionId, text, this.currentProjectDir);
        } catch (error) {
            console.error('Failed to send message:', error);
            showError(error.toString());
            
            if (window.inputPanelController) {
                window.inputPanelController.setGeneratingState(false);
            }
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
        else if (event.ToolCallDetected) {
            this.ensureAssistantMessageCreated();
            const { call_id, name, attrs } = event.ToolCallDetected;
            this.createToolCallCard(call_id, name, attrs);
        }
        else if (event.ToolBodyDelta) {
            const { call_id, text } = event.ToolBodyDelta;
            const toolCard = this.activeToolCalls.get(call_id);
            if (toolCard) {
                const codeEl = toolCard.querySelector('.assistant-message__tool-code');
                if (codeEl) {
                    if (codeEl.textContent === 'Pending...') {
                        codeEl.textContent = '';
                    }
                    codeEl.textContent += text;
                }
            }
            window.assistantMessageController.scrollToBottom();
        }
        else if (event.ToolCallStart) {
            this.ensureAssistantMessageCreated();
            const { call_id, name } = event.ToolCallStart;
            
            // Map the streaming tool card if it exists
            const parts = call_id.split('-');
            if (parts.length >= 3) {
                const turn_index = parts[parts.length - 2];
                const idx = parts[parts.length - 1];
                const stream_id = `stream-${turn_index}-${idx}`;
                const streamCard = this.activeToolCalls.get(stream_id);
                if (streamCard) {
                    // Re-key the active tool call card to the final call_id
                    this.activeToolCalls.delete(stream_id);
                    this.activeToolCalls.set(call_id, streamCard);
                    streamCard.id = `tool-${call_id}`;
                    
                    // No need to create a new card
                    return;
                }
            }
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
                window.inputPanelController.setGeneratingState(false);
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
        else if (event === 'Done' || event.Done) {
            // Turn completed successfully!
            // Collapse thinking block if finished
            if (this.currentThinkingEl) {
                this.currentThinkingEl.classList.remove('thinking-active');
                this.currentThinkingEl.classList.add('collapsed');
            }
            // Highlight the code blocks in the finished streaming message
            if (this.currentAssistantContentEl && window.highlightCodeBlocks) {
                window.highlightCodeBlocks(this.currentAssistantContentEl);
            }
            if (window.inputPanelController) {
                window.inputPanelController.setGeneratingState(false);
            }
            this.resetStreamingState();
            this.loadSessionsList();
        } 
        else if (event.Error) {
            if (window.inputPanelController) {
                window.inputPanelController.setGeneratingState(false);
            }
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
            this.currentThinkingEl.classList.remove('thinking-active');
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
            thinkingCard.className = 'assistant-message__thinking thinking-active';
            thinkingCard.innerHTML = `
                <div class="assistant-message__thinking-header">
                    <img class="assistant-message__thinking-icon" src="./assets/icons/main-content/messages/assistant/thinking.svg" style="filter: invert(0.6); width:14px; height:14px;">
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
    createToolCallCard(callId, name, attrs = "") {
        // If we were thinking, collapse that thinking block.
        if (this.currentThinkingEl) {
            this.currentThinkingEl.classList.remove('thinking-active');
            this.currentThinkingEl.classList.add('collapsed');
            this.currentThinkingEl = null;
        }
        // Force new text/thinking blocks after this tool execution card.
        this.currentAssistantContentEl = null;

        const toolCard = document.createElement('div');
        toolCard.className = 'assistant-message__tool-card';
        toolCard.id = `tool-${callId}`;
        toolCard.dataset.toolName = name; // Cache the tool name on the DOM element for access during updates

        // Parse path from XML attributes and generate the running title (e.g. "Editing ipc.js")
        const path = extractPathFromAttrs(attrs);
        const argsObj = { path };
        const { title: headerTitle, tooltip: tooltipText } = getToolHeaderTitle(name, argsObj, false);
        const tooltipAttr = tooltipText ? `title="${escapeAttribute(tooltipText)}"` : '';

        toolCard.innerHTML = `
            <div class="assistant-message__tool-header">
                <div class="assistant-message__tool-title-wrapper">
                    <span class="assistant-message__tool-icon">
                        <img src="./assets/icons/main-content/messages/assistant/tool.svg" style="filter: invert(0.7); width:14px; height:14px;">
                    </span>
                    <span class="assistant-message__tool-name" ${tooltipAttr}>${escapeHtml(headerTitle)}</span>
                </div>
                <div class="assistant-message__tool-status-wrapper" style="display: flex; align-items: center; gap: 8px;">
                    <span class="assistant-message__tool-status assistant-message__tool-status--running">
                        <svg class="tool-spinner-svg" width="14" height="14" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-dasharray="31.4 31.4" opacity="0.25"/>
                            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" stroke-linecap="round" stroke-dasharray="31.4 31.4" stroke-dashoffset="10">
                                <animateTransform attributeName="transform" type="rotate" dur="0.8s" from="0 12 12" to="360 12 12" repeatCount="indefinite" />
                            </circle>
                        </svg>
                    </span>
                    <img class="assistant-message__tool-chevron" src="./assets/icons/sidebar/chevron-down.svg" style="filter: invert(0.6); width: 14px; height: 14px;">
                </div>
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
            // Retrieve cached tool name from the dataset attribute
            const toolName = toolCard.dataset.toolName || '';
            const isDiffTool = (toolName === 'write' || toolName === 'append' || toolName === 'edit');
            
            let argsObj = parseArgsJson(argsJson);
            
            // Update the tool card header text and tooltip with the parsed path information
            const nameEl = toolCard.querySelector('.assistant-message__tool-name');
            if (nameEl && argsObj) {
                const { title: headerTitle, tooltip: tooltipText } = getToolHeaderTitle(toolName, argsObj, false);
                nameEl.textContent = headerTitle;
                if (tooltipText) {
                    nameEl.setAttribute('title', tooltipText);
                }
            }

            if (isDiffTool) {
                if (argsObj) {
                    // Update diff stats (+N, -M counts) in the tool card header badge
                    updateToolCardDiffStats(toolCard, toolName, argsObj);
                    
                    // Replace the details element content with beautiful diff/added lines HTML
                    const detailsEl = toolCard.querySelector('.assistant-message__tool-details');
                    if (detailsEl) {
                        detailsEl.innerHTML = renderToolDiffHTML(toolName, argsObj);
                    }
                }
            } else {
                // Classic code block rendering for non-diff/regular tools
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
                    statusEl.innerHTML = '<img class="assistant-message__tool-status-icon assistant-message__tool-status-icon--completed" src="./assets/icons/main-content/messages/assistant/circle-check.svg">';
                }
            }
            
            // Swap header title text to its completed past-tense equivalent (e.g. "Editing main.js" -> "Edited main.js")
            const nameEl = toolCard.querySelector('.assistant-message__tool-name');
            if (nameEl) {
                const text = nameEl.textContent;
                if (text.startsWith('Editing ')) {
                    nameEl.textContent = text.replace('Editing ', 'Edited ');
                } else if (text.startsWith('Writing ')) {
                    nameEl.textContent = text.replace('Writing ', 'Wrote ');
                } else if (text.startsWith('Appending ')) {
                    nameEl.textContent = text.replace('Appending ', 'Appended ');
                } else if (text.startsWith('Reading ')) {
                    nameEl.textContent = text.replace('Reading ', 'Read ');
                } else if (text.startsWith('Deleting ')) {
                    nameEl.textContent = text.replace('Deleting ', 'Deleted ');
                } else if (text.startsWith('Listing ')) {
                    nameEl.textContent = text.replace('Listing ', 'Listed ');
                } else if (text.startsWith('Searching ')) {
                    nameEl.textContent = text.replace('Searching ', 'Searched ');
                } else if (text.startsWith('Executing ')) {
                    nameEl.textContent = text.replace('Executing ', 'Executed ');
                } else if (text.startsWith('Asking ')) {
                    nameEl.textContent = text.replace('Asking ', 'Asked ');
                } else if (text.startsWith('Fetching ')) {
                    nameEl.textContent = text.replace('Fetching ', 'Fetched ');
                } else if (text.startsWith('Creating ')) {
                    nameEl.textContent = text.replace('Creating ', 'Created ');
                } else if (text.startsWith('Updating ')) {
                    nameEl.textContent = text.replace('Updating ', 'Updated ');
                } else if (text.startsWith('Running ')) {
                    nameEl.textContent = text.replace('Running ', 'Finished ');
                }
            }

            // Retrieve cached tool name from the dataset attribute
            const toolName = toolCard.dataset.toolName || '';
            const isDiffTool = (toolName === 'write' || toolName === 'append' || toolName === 'edit');
            
            // For write, append, and edit tools, we suppress the Result block entirely.
            // A success checkmark or failure badge is already displayed in the card header.
            if (!isDiffTool) {
                // Append result section for normal/default tools
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
                        <pre class="assistant-message__tool-code">${escapeHtml(cleanResult)}</pre>
                    `;
                    detailsEl.appendChild(resultSection);
                }
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
            this.currentThinkingEl.classList.remove('thinking-active');
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
            pathInfo = ` on <code style="background: rgba(0,0,0,0.3); padding: 2px 4px; border-radius: 3px;">${escapeHtml(path)}</code>`;
        }
        
        permCard.innerHTML = `
            <div class="assistant-message__permission-header">
                <img class="assistant-message__permission-warning-icon" src="./assets/icons/sidebar/settings.svg" style="filter: invert(0.8) sepia(1) saturate(5) hue-rotate(5deg); width:18px; height:18px;">
                <span class="assistant-message__permission-title">Permission Requested</span>
            </div>
            <div class="assistant-message__permission-body">
                <div class="assistant-message__permission-reason" style="margin-bottom: 12px; font-size:13px; line-height:18px;">
                    The model requests permission to execute <strong>${escapeHtml(tool)}</strong>${pathInfo}.<br>
                    <span style="color: #bbbbbb; font-style: italic;">Reason: ${escapeHtml(reason)}</span>
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
            this.currentThinkingEl.classList.remove('thinking-active');
            this.currentThinkingEl.classList.add('collapsed');
            this.currentThinkingEl = null;
        }
        // Force new text/thinking blocks after this card.
        this.currentAssistantContentEl = null;

        const askCard = document.createElement('div');
        askCard.className = 'assistant-message__ask-card';
        askCard.id = `ask-${id}`;
        
        let optionsHtml = options.map((opt, index) => {
            return `<button class="btn-ask-option" data-option="${escapeAttribute(opt)}">${escapeHtml(opt)}</button>`;
        }).join('');

        askCard.innerHTML = `
            <div class="assistant-message__ask-header">
                <img class="assistant-message__ask-icon" src="./assets/icons/sidebar/new-chat.svg" style="filter: invert(0.6) sepia(1) saturate(5) hue-rotate(180deg); width:18px; height:18px;">
                <span class="assistant-message__ask-title">Question Prompt</span>
            </div>
            <div class="assistant-message__ask-body">
                <div class="assistant-message__ask-question">${escapeHtml(question)}</div>
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
