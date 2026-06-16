'use strict';

/**
 * ipc.js
 *
 * Centralized IPC (Inter-Process Communication) module for Tauri backend calls.
 * All frontend-to-backend communication goes through this module.
 *
 * Uses the Tauri invoke API to call Rust command handlers.
 */

/**
 * Check if Tauri API is available
 * @returns {boolean}
 */
export function isTauriAvailable() {
    return typeof window !== 'undefined' && 
           typeof window.__TAURI__ !== 'undefined' &&
           typeof window.__TAURI__.core !== 'undefined';
}

/**
 * Invoke a Tauri command
 * @param {string} command - The command name
 * @param {Object} [args] - Optional arguments
 * @returns {Promise<any>}
 */
async function invoke(command, args = {}) {
    if (!isTauriAvailable()) {
        throw new Error('Tauri API is not available');
    }
    
    try {
        const result = await window.__TAURI__.core.invoke(command, args);
        return result;
    } catch (error) {
        console.error(`IPC error calling ${command}:`, error);
        throw error;
    }
}

/**
 * Normalize a value into a trimmed string.
 *
 * Tauri command payloads are serialized through JSON, so we keep the values
 * explicit and string-based here instead of letting `undefined` disappear from
 * the request object and break the backend deserializer.
 *
 * @param {unknown} value - Any incoming value from the UI layer.
 * @returns {string}
 */
function normalizeString(value) {
    return String(value ?? '').trim();
}

/**
 * Build the payload expected by the Rust `discover_models` command.
 *
 * The backend command takes a single `request` object, so the frontend must
 * wrap the fields rather than sending them flat at the top level.
 *
 * @param {Object} request - Raw request data from the UI.
 * @returns {{ providerId: string, apiBase: string, apiKey: string }}
 */
function buildDiscoverModelsRequest(request) {
    if (!request || typeof request !== 'object' || Array.isArray(request)) {
        throw new TypeError('discoverModels expects a request object.');
    }

    return {
        providerId: normalizeString(request.providerId),
        apiBase: normalizeString(request.apiBase),
        apiKey: normalizeString(request.apiKey),
    };
}

/**
 * Build the payload expected by the Rust `save_provider_setup` command.
 *
 * This mirrors the backend DTO exactly so the IPC bridge stays explicit and
 * easy to debug when the provider setup flow changes later.
 *
 * @param {Object} request - Raw request data from the UI.
 * @returns {{ providerId: string, apiBase: string, apiKey: string, model: string }}
 */
function buildSaveProviderRequest(request) {
    if (!request || typeof request !== 'object' || Array.isArray(request)) {
        throw new TypeError('saveProviderSetup expects a request object.');
    }

    return {
        providerId: normalizeString(request.providerId),
        apiBase: normalizeString(request.apiBase),
        apiKey: normalizeString(request.apiKey),
        model: normalizeString(request.model),
    };
}

// ══════════════════════════════════════════════════════════════════════════════
// MODEL PROVIDER COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Get list of all available model providers
 * @returns {Promise<Array<Object>>}
 */
export async function getModelProviders() {
    return await invoke('get_model_providers');
}

/**
 * Get detailed setup for a specific provider
 * @param {string} providerId - The provider ID (e.g., 'anthropic', 'open_ai')
 * @returns {Promise<Object>}
 */
export async function getModelProviderSetup(providerId) {
    return await invoke('get_model_provider_setup', { providerId });
}

/**
 * Discover available models for a provider
 * @param {Object} request - Provider discovery request payload
 * @param {string} request.providerId - The provider ID
 * @param {string} request.apiBase - API base URL
 * @param {string} request.apiKey - API key
 * @returns {Promise<Object>} - { models: Array, activeModel: string }
 */
export async function discoverModels(request) {
    const payload = buildDiscoverModelsRequest(request);
    return await invoke('discover_models', { request: payload });
}

/**
 * Save provider configuration and activate it
 * @param {Object} request - Provider setup payload
 * @param {string} request.providerId - The provider ID
 * @param {string} request.apiBase - API base URL
 * @param {string} request.apiKey - API key
 * @param {string} request.model - Model ID
 * @returns {Promise<Object>} - { model: string }
 */
export async function saveProviderSetup(request) {
    const payload = buildSaveProviderRequest(request);
    return await invoke('save_provider_setup', { request: payload });
}

/**
 * Get the currently active provider configuration
 * @returns {Promise<Object|null>}
 */
export async function getActiveProvider() {
    return await invoke('get_active_provider');
}

// ══════════════════════════════════════════════════════════════════════════════
// PERMISSION COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Get list of allowed directories and the default workspace
 * @returns {Promise<Object>} - { workspaceDirectory: string, directories: string[] }
 */
export async function getAllowedDirectories() {
    return await invoke('get_allowed_directories');
}

/**
 * Add a new allowed directory
 * @param {string} directory - Directory path to add
 * @returns {Promise<Object>} - Updated allowed directories response
 */
export async function addAllowedDirectory(directory) {
    return await invoke('add_allowed_directory', { directory: normalizeString(directory) });
}

/**
 * Remove an allowed directory
 * @param {string} directory - Directory path to remove
 * @returns {Promise<Object>} - Updated allowed directories response
 */
export async function removeAllowedDirectory(directory) {
    return await invoke('remove_allowed_directory', { directory: normalizeString(directory) });
}

/**
 * Get permission rows for a given scope and directory
 * @param {string} scope - 'owner' or 'external'
 * @param {string|null} [directory=null] - Optional directory path for directory-scoped permissions
 * @returns {Promise<Array<Object>>}
 */
export async function getPermissionRows(scope, directory = null) {
    return await invoke('get_permission_rows', {
        scope: normalizeString(scope),
        directory: directory ? normalizeString(directory) : null,
    });
}

/**
 * Update the permission mode of a key
 * @param {string} scope - 'owner' or 'external'
 * @param {string|null} directory - Optional directory path
 * @param {string} key - Permission key (tool or group name)
 * @param {string|null} mode - Permission mode ('allow', 'ask', 'deny') or null to inherit/reset
 * @returns {Promise<void>}
 */
export async function updatePermissionMode(scope, directory, key, mode) {
    return await invoke('update_permission_mode', {
        scope: normalizeString(scope),
        directory: directory ? normalizeString(directory) : null,
        key: normalizeString(key),
        mode: mode ? normalizeString(mode) : null,
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// SESSION COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * List all historical sessions saved on the system
 * @returns {Promise<Array<Object>>}
 */
export async function listSessions() {
    return await invoke('list_sessions');
}

/**
 * Get message history for a session
 * @param {string} sessionId - The session ID
 * @returns {Promise<Array<Object>>}
 */
export async function getSessionHistory(sessionId) {
    return await invoke('get_session_history', { sessionId: normalizeString(sessionId) });
}

/**
 * Send a message to a session runner (starts/resumes it in background)
 * @param {string} sessionId - The session ID
 * @param {string} message - User message
 * @param {string|null} [projectDir=null] - Project directory path if in PROJECT mode
 * @returns {Promise<void>}
 */
export async function sendMessage(sessionId, message, projectDir = null) {
    return await invoke('send_message', {
        sessionId: normalizeString(sessionId),
        message: normalizeString(message),
        projectDir: projectDir ? normalizeString(projectDir) : null,
    });
}

/**
 * Gracefully cancel the running session
 * @param {string} sessionId - The session ID
 * @returns {Promise<void>}
 */
export async function cancelSession(sessionId) {
    return await invoke('cancel_session', { sessionId: normalizeString(sessionId) });
}

/**
 * Approve a pending Ask-mode permission request
 * @param {string} sessionId - The session ID
 * @param {string} id - The approval request ID
 * @returns {Promise<void>}
 */
export async function approveToolCall(sessionId, id) {
    return await invoke('approve_tool_call', {
        sessionId: normalizeString(sessionId),
        id: normalizeString(id),
    });
}

/**
 * Deny a pending Ask-mode permission request
 * @param {string} sessionId - The session ID
 * @param {string} id - The approval request ID
 * @returns {Promise<void>}
 */
export async function denyToolCall(sessionId, id) {
    return await invoke('deny_tool_call', {
        sessionId: normalizeString(sessionId),
        id: normalizeString(id),
    });
}

/**
 * Send the user's answer to a suspended `ask` tool call
 * @param {string} sessionId - The session ID
 * @param {string} id - The ask request ID
 * @param {string} answer - The user's answer
 * @returns {Promise<void>}
 */
export async function answerAsk(sessionId, id, answer) {
    return await invoke('answer_ask', {
        sessionId: normalizeString(sessionId),
        id: normalizeString(id),
        answer: normalizeString(answer),
    });
}


/**
 * Open native OS folder picker and register project directory.
 * @returns {Promise<string|null>} - Selected path or null if cancelled
 */
export async function openProjectFolder() {
    return await invoke('open_project_folder');
}

/**
 * Get canonical path of the default workspace directory.
 * @returns {Promise<string>}
 */
export async function getDefaultWorkspace() {
    return await invoke('get_default_workspace');
}

/**
 * Renders raw markdown into HTML on the backend.
 * @param {string} markdown - The markdown content to render.
 * @returns {Promise<string>} - The rendered HTML string.
 */
export async function renderMarkdown(markdown) {
    return await invoke('render_markdown', { markdown });
}

/**
 * Delete a specific session by its ID.
 * @param {string} sessionId - The session ID to delete.
 * @returns {Promise<void>}
 */
export async function deleteSession(sessionId) {
    return await invoke('delete_session', { sessionId: normalizeString(sessionId) });
}

/**
 * Delete a project and all its associated session databases.
 * @param {string} projectPath - The project workspace path to delete.
 * @returns {Promise<void>}
 */
export async function deleteProject(projectPath) {
    return await invoke('delete_project', { projectPath: normalizeString(projectPath) });
}

// ══════════════════════════════════════════════════════════════════════════════
// MEMORY COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Retrieve all memories from the global database
 * @returns {Promise<Array<Object>>}
 */
export async function getMemories() {
    return await invoke('get_memories');
}

/**
 * Add a new memory entry
 * @param {string} content - Text content of the memory
 * @returns {Promise<Object>} - The newly created memory item
 */
export async function addMemory(content) {
    return await invoke('add_memory', { content: normalizeString(content) });
}

/**
 * Update an existing memory entry's text content by its ID
 * @param {number} id - Memory entry ID
 * @param {string} content - Updated text content
 * @returns {Promise<void>}
 */
export async function updateMemory(id, content) {
    return await invoke('update_memory', { id: Number(id), content: normalizeString(content) });
}

/**
 * Delete a memory entry from the database by its ID
 * @param {number} id - Memory entry ID to delete
 * @returns {Promise<void>}
 */
export async function deleteMemory(id) {
    return await invoke('delete_memory', { id: Number(id) });
}

// ══════════════════════════════════════════════════════════════════════════════
// TERMINAL COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Spawn a new terminal session
 * @param {string} id - Unique terminal ID (e.g. 'term_1')
 * @param {number} cols - Columns
 * @param {number} rows - Rows
 * @param {string|null} workdir - Starting directory
 * @returns {Promise<void>}
 */
export async function createTerminal(id, cols, rows, workdir = null) {
    return await invoke('create_terminal', {
        id: normalizeString(id),
        cols: Number(cols),
        rows: Number(rows),
        workdir: workdir ? normalizeString(workdir) : null,
    });
}

/**
 * Write characters or commands to a terminal session
 * @param {string} id - Terminal ID
 * @param {string} input - Input string
 * @returns {Promise<void>}
 */
export async function writeTerminal(id, input) {
    return await invoke('write_terminal', {
        id: normalizeString(id),
        input,
    });
}

/**
 * Resize a terminal session grid cols/rows
 * @param {string} id - Terminal ID
 * @param {number} cols - New columns
 * @param {number} rows - New rows
 * @returns {Promise<void>}
 */
export async function resizeTerminal(id, cols, rows) {
    return await invoke('resize_terminal', {
        id: normalizeString(id),
        cols: Number(cols),
        rows: Number(rows),
    });
}

/**
 * Close and terminate a terminal session
 * @param {string} id - Terminal ID
 * @returns {Promise<void>}
 */
export async function closeTerminal(id) {
    return await invoke('close_terminal', {
        id: normalizeString(id),
    });
}

// ══════════════════════════════════════════════════════════════════════════════
// GIT DIFF COMMANDS
// ══════════════════════════════════════════════════════════════════════════════

/**
 * Fetch general git stats (insertions/deletions) for the quick changes badge.
 * Hey friend! This connects directly to get_git_diff_stats on the Rust backend.
 * @param {string|null} [projectDir=null] - Workspace project folder path
 * @returns {Promise<{hasRepo: boolean, insertions: number, deletions: number}>}
 */
export async function getGitDiffStats(projectDir = null) {
    return await invoke('get_git_diff_stats', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
    });
}

/**
 * Fetch detailed file-by-file status list and diff hunks to build the panel tree.
 * @param {string|null} [projectDir=null] - Workspace project folder path
 * @returns {Promise<Object>} Detailed RepositoryDiff DTO from Rust
 */
export async function getGitDiffDetails(projectDir = null) {
    return await invoke('get_git_diff_details', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
    });
}

/**
 * Stage a modified or untracked file to index.
 * @param {string|null} projectDir - Workspace project folder path
 * @param {string} relativePath - The file path relative to repo root
 * @returns {Promise<void>}
 */
export async function stageGitFile(projectDir, relativePath) {
    return await invoke('stage_git_file', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
        relativePath: normalizeString(relativePath),
    });
}

/**
 * Unstage a file by resetting it back to HEAD.
 * @param {string|null} projectDir - Workspace project folder path
 * @param {string} relativePath - The file path relative to repo root
 * @returns {Promise<void>}
 */
export async function unstageGitFile(projectDir, relativePath) {
    return await invoke('unstage_git_file', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
        relativePath: normalizeString(relativePath),
    });
}

/**
 * Discard unstaged changes to a file in the workspace.
 * @param {string|null} projectDir - Workspace project folder path
 * @param {string} relativePath - The file path relative to repo root
 * @returns {Promise<void>}
 */
export async function revertGitFile(projectDir, relativePath) {
    return await invoke('revert_git_file', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
        relativePath: normalizeString(relativePath),
    });
}

/**
 * Stage all unstaged modifications and untracked files.
 * @param {string|null} projectDir - Workspace project folder path
 * @returns {Promise<void>}
 */
export async function stageAllGitFiles(projectDir) {
    return await invoke('stage_all_git_files', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
    });
}

/**
 * Revert all unstaged changes to tracked files in the workspace.
 * @param {string|null} projectDir - Workspace project folder path
 * @returns {Promise<void>}
 */
export async function revertAllGitFiles(projectDir) {
    return await invoke('revert_all_git_files', {
        projectDir: projectDir ? normalizeString(projectDir) : null,
    });
}



