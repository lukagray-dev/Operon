// IPC helpers for the Memory settings panel.
// Wraps all four Tauri commands with typed return signatures.

import { invokeIpc } from '../../shared/ipc.js';
import type { MemoryEntry, MemoryListResponse } from './types.js';

/** Fetches a paginated list of memories from the store. */
export async function memoryListIpc(
  limit: number,
  offset: number
): Promise<MemoryListResponse | null> {
  return invokeIpc<MemoryListResponse>('memory_list', { limit, offset });
}

/** Creates a new memory entry and returns it. */
export async function memoryAddIpc(
  content: string,
  tags: string[]
): Promise<MemoryEntry | null> {
  return invokeIpc<MemoryEntry>('memory_add', { content, tags });
}

/** Partially updates an existing memory (only provided fields change). */
export async function memoryEditIpc(
  id: string,
  content: string | null,
  tags: string[] | null
): Promise<MemoryEntry | null> {
  return invokeIpc<MemoryEntry>('memory_edit', { id, content, tags });
}

/** Deletes a memory by id. Returns the deleted id and remaining count. */
export async function memoryDeleteIpc(
  id: string
): Promise<{ id: string; remaining: number } | null> {
  return invokeIpc<{ id: string; remaining: number }>('memory_delete', { id });
}
