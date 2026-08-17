// TypeScript interfaces for Source Control & Git Diff Right Sidebar

export type RightSidebarPanel = 'git' | 'todos';

export interface GitDiffLine {
  line_type: string; // "+" for addition, "-" for deletion, " " for context
  content: string;
  old_line_num: string;
  new_line_num: string;
}

export interface GitDiffHunk {
  header: string;
  lines: GitDiffLine[];
}

export interface GitFileDiff {
  path: string;
  file_name: string;
  dir_path: string;
  status: 'modified' | 'added' | 'deleted' | 'untracked' | 'renamed' | string;
  insertions: number;
  deletions: number;
  hunks: GitDiffHunk[];
  is_expanded: boolean;
}

export interface GitRepositoryInfo {
  name: string;
  path: string;
  branch: string;
  is_active: boolean;
  has_changes: boolean;
}

export interface GitGraphCommit {
  hash: string;
  short_hash: string;
  message: string;
  author: string;
  branch_tag: string;
  is_head: boolean;
  is_local: boolean;
}

export interface GitDiffDetails {
  has_repo: boolean;
  repo_name: string;
  current_branch: string;
  total_insertions: number;
  total_deletions: number;
  unstaged_files: GitFileDiff[];
  staged_files: GitFileDiff[];
}

export interface ContextMenuItem {
  id: string;
  label: string;
  shortcut: string;
  has_submenu: boolean;
  is_separator: boolean;
  is_disabled: boolean;
}
