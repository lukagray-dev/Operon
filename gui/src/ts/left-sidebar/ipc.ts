// Left Sidebar IPC callers

import { invokeIpc } from '../shared/ipc.js';
import type { SidebarData, SidebarProject } from './types.js';

export async function querySidebarData(searchQuery = ''): Promise<SidebarData> {
  const res = await invokeIpc<SidebarData>('query_sidebar_data', { searchQuery });
  return res || { chats: [], projects: [], active_session_id: null };
}

export async function deleteSessionIpc(sessionId: string): Promise<void> {
  await invokeIpc('delete_session', { sessionId });
}

export async function deleteProjectIpc(projectPath: string): Promise<void> {
  await invokeIpc('delete_project', { projectPath });
}

export async function openProjectPickerIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('open_project_picker');
}

export async function createNewSessionIpc(sessionId?: string, projectPath?: string): Promise<string> {
  const res = await invokeIpc<string>('create_new_session', { sessionId, projectPath });
  return res || `session-${Date.now()}`;
}

export async function setActiveSessionIpc(sessionId: string | null, projectPath: string | null): Promise<void> {
  await invokeIpc('set_active_session', { sessionId, projectPath });
}

export async function renameSessionIpc(sessionId: string, newTitle: string): Promise<void> {
  await invokeIpc('rename_session', { sessionId, newTitle });
}

export async function forkSessionIpc(sessionId: string, untilTurnIndex?: number): Promise<string> {
  const res = await invokeIpc<string>('fork_session', {
    sessionId,
    untilTurnIndex: untilTurnIndex !== undefined ? untilTurnIndex : null,
  });
  return res || sessionId;
}

export async function moveSessionIpc(sessionId: string, targetWorkspace: string): Promise<void> {
  await invokeIpc('move_session', { sessionId, targetWorkspace });
}

export async function queryWhatsAppContactsIpc(): Promise<SidebarProject[]> {
  const res = await invokeIpc<SidebarProject[]>('query_whatsapp_contacts');
  return res || [];
}

export async function queryTelegramContactsIpc(): Promise<SidebarProject[]> {
  const res = await invokeIpc<SidebarProject[]>('query_telegram_contacts');
  return res || [];
}

export async function queryDiscordContactsIpc(): Promise<SidebarProject[]> {
  const res = await invokeIpc<SidebarProject[]>('query_discord_contacts');
  return res || [];
}

