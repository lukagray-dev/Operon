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
 * Renders raw markdown into HTML on the backend.
 * @param {string} markdown - The markdown content to render.
 * @returns {Promise<string>} - The rendered HTML string.
 */
export async function renderMarkdown(markdown) {
    return await invoke('render_markdown', { markdown });
}

