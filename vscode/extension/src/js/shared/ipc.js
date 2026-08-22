// ============================================================================
// Type-safe VS Code Webview IPC Bridge
//
// Hey friend! This module provides the communication bridge between our
// Webview UI (running in an isolated browser iframe) and the VS Code Extension
// Host (running in Node.js), which in turn communicates with the Rust backend.
//
// It exports the exact same API contract (`invokeIpc` and `listenIpcEvent`)
// as the Tauri desktop GUI, enabling shared code and symmetrical domain modules!
// ============================================================================
// Ensure acquireVsCodeApi is called once and cached for the webview lifecycle
let vsCodeApiInstance;
function getVsCodeApi() {
    if (typeof vsCodeApiInstance !== 'undefined') {
        return vsCodeApiInstance;
    }
    if (typeof acquireVsCodeApi === 'function') {
        try {
            vsCodeApiInstance = acquireVsCodeApi();
            return vsCodeApiInstance;
        }
        catch {
            // In browser preview or already acquired
        }
    }
    return undefined;
}
// Monotonically increasing request identifier
let nextRequestId = 1;
// Map of pending RPC promise resolvers: Map<id, { resolve, reject }>
const pendingRequests = new Map();
// Map of registered event listeners: Map<eventName, Set<handler>>
const eventListeners = new Map();
// Listen for incoming response and event messages from the Extension Host
if (typeof window !== 'undefined') {
    window.addEventListener('message', (event) => {
        const msg = event.data;
        if (!msg || typeof msg !== 'object')
            return;
        // Handle RPC response matching a pending invoke call
        if (msg.type === 'response' && typeof msg.id === 'number') {
            const pending = pendingRequests.get(msg.id);
            if (pending) {
                pendingRequests.delete(msg.id);
                if (msg.error) {
                    pending.reject(new Error(msg.error));
                }
                else {
                    pending.resolve(msg.result);
                }
            }
        }
        // Handle streaming or broadcast events from the backend
        else if (msg.type === 'event' && typeof msg.event === 'string') {
            const listeners = eventListeners.get(msg.event);
            if (listeners) {
                listeners.forEach((handler) => {
                    try {
                        handler(msg.payload);
                    }
                    catch (err) {
                        console.error(`[IPC] Error in event listener for '${msg.event}':`, err);
                    }
                });
            }
        }
    });
}
/**
 * Invokes a backend command with optional arguments and returns a Promise
 * resolving with the typed response data.
 *
 * @param cmd The command name (e.g. 'submit_prompt', 'open_settings_window')
 * @param args Key-value dictionary of arguments passed to the backend command
 */
export async function invokeIpc(cmd, args) {
    const vscode = getVsCodeApi();
    if (!vscode) {
        console.warn(`[IPC] VS Code API unavailable. Mocked response for: ${cmd}`);
        return null;
    }
    const id = nextRequestId++;
    return new Promise((resolve, reject) => {
        pendingRequests.set(id, { resolve, reject });
        vscode.postMessage({
            id,
            type: 'invoke',
            cmd,
            args: args || {},
        });
    });
}
/**
 * Subscribes to a streaming or notification event from the backend.
 * Returns an asynchronous cleanup function that unregisters the handler.
 *
 * @param event The event identifier (e.g. 'agent-event', 'agent-finished')
 * @param handler Callback receiving the event payload
 */
export async function listenIpcEvent(event, handler) {
    let listeners = eventListeners.get(event);
    if (!listeners) {
        listeners = new Set();
        eventListeners.set(event, listeners);
    }
    listeners.add(handler);
    return () => {
        const current = eventListeners.get(event);
        if (current) {
            current.delete(handler);
            if (current.size === 0) {
                eventListeners.delete(event);
            }
        }
    };
}
//# sourceMappingURL=ipc.js.map