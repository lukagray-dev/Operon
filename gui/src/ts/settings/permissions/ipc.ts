// Permissions Settings IPC Wrappers

import { invokeIpc } from '../../shared/ipc.js';
import type {
  AllowedDirectories,
  PermissionItem,
  UpdatePermissionRequest,
} from './types.js';

export async function getAllowedDirectoriesIpc(): Promise<AllowedDirectories> {
  const res = await invokeIpc<AllowedDirectories>('get_allowed_directories');
  return res || { directories: [], workspace_directory: '' };
}

export async function addAllowedDirectoryIpc(path: string): Promise<void> {
  await invokeIpc('add_allowed_directory', { path });
}

export async function removeAllowedDirectoryIpc(path: string): Promise<void> {
  await invokeIpc('remove_allowed_directory', { path });
}

export async function pickAllowedDirectoryDialogIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('pick_allowed_directory_dialog');
}

export async function getPermissionItemsIpc(
  scope: string,
  directory?: string
): Promise<PermissionItem[]> {
  const res = await invokeIpc<PermissionItem[]>('get_permission_items', {
    scope,
    directory: directory || undefined,
  });
  return res || [];
}

export async function updatePermissionModeIpc(
  request: UpdatePermissionRequest
): Promise<void> {
  await invokeIpc('update_permission_mode', { request });
}
