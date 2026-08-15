// Settings Window IPC wrappers

import { invokeIpc } from '../shared/ipc.js';

/**
 * Invokes the backend to open or focus the standalone Settings window.
 */
export async function openSettingsWindowIpc(): Promise<void> {
  await invokeIpc('open_settings_window');
}

/**
 * Invokes the backend to close the standalone Settings window.
 */
export async function closeSettingsWindowIpc(): Promise<void> {
  await invokeIpc('close_settings_window');
}
