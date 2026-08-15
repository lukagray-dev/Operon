// About Settings IPC Wrappers

import { invokeIpc } from '../../shared/ipc.js';
import type { AboutSystemInfo } from './types.js';

export async function getAboutSystemInfoIpc(): Promise<AboutSystemInfo | null> {
  return await invokeIpc<AboutSystemInfo>('get_about_system_info');
}

export async function openExternalUrlIpc(url: string): Promise<void> {
  await invokeIpc('open_external_url', { url });
}
