// ============================================================================
// Main Content Topbar IPC Wrappers for VS Code
// ============================================================================

import { invokeIpc } from '../../shared/ipc.js';
import type { TopbarDataDto } from './types.js';

/**
 * Fetches the topbar header information for the current session and workspace.
 */
export async function getTopbarInfoIpc(
  sessionId?: string,
  workspacePath?: string
): Promise<TopbarDataDto> {
  const res = await invokeIpc<TopbarDataDto>('get_topbar_info', {
    sessionId,
    workspacePath,
  });
  return (
    res || {
      title: 'New Session',
      is_project: false,
      project_name: null,
      unfinished_todo_count: 0,
      total_todo_count: 0,
    }
  );
}
