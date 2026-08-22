// ============================================================================
// Session Tasks / Todo Panel TypeScript Type Definitions
//
// Hey friend! This file holds all the data contracts and type definitions
// used by our right-sidebar Todo panel in VS Code.
// ============================================================================

export type TodoStatus = 'pending' | 'in_progress' | 'completed';
export type TodoPriority = 'high' | 'medium' | 'low';
export type TodoFilter = 'all' | 'pending' | 'completed';

export interface TodoItemDto {
  id: string;
  content: string;
  status: TodoStatus;
  priority: TodoPriority;
}
