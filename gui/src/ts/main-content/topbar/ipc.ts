// Main Content Topbar IPC Callers

import { invokeIpc } from '../../shared/ipc.js';
import type { GitDiffStats, TopbarData } from './types.js';

export async function getTopbarInfoIpc(
  sessionId?: string,
  workspacePath?: string
): Promise<TopbarData> {
  const res = await invokeIpc<TopbarData>('get_topbar_info', {
    sessionId: sessionId || null,
    workspacePath: workspacePath || null,
  });

  return (
    res || {
      title: 'New Session',
      is_project: false,
    }
  );
}

export async function getGitDiffStatsIpc(workspacePath?: string): Promise<GitDiffStats> {
  const res = await invokeIpc<GitDiffStats>('get_git_diff_stats', {
    workspacePath: workspacePath || null,
  });

  return (
    res || {
      insertions: 0,
      deletions: 0,
      files_changed: 0,
      is_git_repo: false,
    }
  );
}
