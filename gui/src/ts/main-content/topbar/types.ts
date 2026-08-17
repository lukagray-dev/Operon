// TypeScript interfaces for Main Content Topbar

export interface GitDiffStats {
  insertions: number;
  deletions: number;
  files_changed: number;
  is_git_repo: boolean;
}

export interface TopbarData {
  title: string;
  is_project: boolean;
  project_name?: string;
  git_stats?: GitDiffStats;
  unfinished_todo_count?: number;
  total_todo_count?: number;
}
