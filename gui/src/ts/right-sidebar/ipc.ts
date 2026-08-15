// IPC invocation wrappers for Source Control & Git Diff operations

import { invokeIpc } from '../shared/ipc.js';
import type {
  GitDiffDetails,
  GitGraphCommit,
  GitRepositoryInfo,
} from './types.js';

export async function getGitDiffDetailsIpc(workspacePath?: string): Promise<GitDiffDetails> {
  const res = await invokeIpc<GitDiffDetails>('get_git_diff_details', { workspacePath });
  return (
    res || {
      has_repo: false,
      repo_name: '',
      current_branch: 'main',
      total_insertions: 0,
      total_deletions: 0,
      unstaged_files: [],
      staged_files: [],
    }
  );
}

export async function getGitCommitGraphIpc(
  workspacePath?: string,
  limit?: number,
  skip?: number
): Promise<GitGraphCommit[]> {
  const res = await invokeIpc<GitGraphCommit[]>('get_git_commit_graph', {
    workspacePath,
    limit,
    skip,
  });
  return res || [];
}

export async function getWorkspaceRepositoriesIpc(
  workspacePath?: string
): Promise<GitRepositoryInfo[]> {
  const res = await invokeIpc<GitRepositoryInfo[]>('get_workspace_repositories', {
    workspacePath,
  });
  return res || [];
}

export async function gitStageFileIpc(relPath: string, workspacePath?: string): Promise<void> {
  await invokeIpc('git_stage_file', { relPath, workspacePath });
}

export async function gitUnstageFileIpc(relPath: string, workspacePath?: string): Promise<void> {
  await invokeIpc('git_unstage_file', { relPath, workspacePath });
}

export async function gitRevertFileIpc(relPath: string, workspacePath?: string): Promise<void> {
  await invokeIpc('git_revert_file', { relPath, workspacePath });
}

export async function gitStageAllFilesIpc(workspacePath?: string): Promise<void> {
  await invokeIpc('git_stage_all_files', { workspacePath });
}

export async function gitUnstageAllFilesIpc(workspacePath?: string): Promise<void> {
  await invokeIpc('git_unstage_all_files', { workspacePath });
}

export async function gitRevertAllFilesIpc(workspacePath?: string): Promise<void> {
  await invokeIpc('git_revert_all_files', { workspacePath });
}

export async function gitCommitChangesIpc(
  message: string,
  amend = false,
  workspacePath?: string
): Promise<string> {
  const res = await invokeIpc<string>('git_commit_changes', {
    message,
    amend,
    workspacePath,
  });
  return res || '';
}

export async function gitGenerateCommitMessageIpc(workspacePath?: string): Promise<string> {
  const res = await invokeIpc<string>('git_generate_commit_message', { workspacePath });
  return res || 'chore: update workspace files';
}

export async function gitPushChangesIpc(
  remote?: string,
  branch?: string,
  workspacePath?: string
): Promise<void> {
  await invokeIpc('git_push_changes', { remote, branch, workspacePath });
}

export async function gitPullChangesIpc(
  remote?: string,
  branch?: string,
  workspacePath?: string
): Promise<void> {
  await invokeIpc('git_pull_changes', { remote, branch, workspacePath });
}

export async function gitFetchChangesIpc(
  remote?: string,
  workspacePath?: string
): Promise<void> {
  await invokeIpc('git_fetch_changes', { remote, workspacePath });
}

export async function gitCreateBranchIpc(name: string, workspacePath?: string): Promise<void> {
  await invokeIpc('git_create_branch', { name, workspacePath });
}

export async function gitSwitchBranchIpc(name: string, workspacePath?: string): Promise<void> {
  await invokeIpc('git_switch_branch', { name, workspacePath });
}

export async function gitDeleteBranchIpc(name: string, workspacePath?: string): Promise<void> {
  await invokeIpc('git_delete_branch', { name, workspacePath });
}
