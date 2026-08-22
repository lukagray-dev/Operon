// ============================================================================
// Main Content Topbar Type Definitions for VS Code
// ============================================================================

export interface TopbarDataDto {
  title: string;
  is_project: boolean;
  project_name: string | null;
  unfinished_todo_count: number;
  total_todo_count: number;
}
