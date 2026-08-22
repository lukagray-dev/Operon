// ============================================================================
// Session Tasks / Todo Panel IPC Wrappers for VS Code
// ============================================================================

import { invokeIpc } from '../shared/ipc.js';
import type { TodoItemDto } from './types.js';

/**
 * Fetches all todo items for the given session ID.
 */
export async function getSessionTodosIpc(sessionId: string): Promise<TodoItemDto[]> {
  const res = await invokeIpc<TodoItemDto[]>('get_session_todos', { sessionId });
  return res || [];
}

/**
 * Updates the status of a specific todo item in the session.
 */
export async function updateSessionTodoStatusIpc(
  sessionId: string,
  todoId: string,
  status: string
): Promise<TodoItemDto[]> {
  const res = await invokeIpc<TodoItemDto[]>('update_session_todo_status', {
    sessionId,
    todoId,
    status,
  });
  return res || [];
}

/**
 * Deletes a specific todo item from the session.
 */
export async function deleteSessionTodoIpc(
  sessionId: string,
  todoId: string
): Promise<TodoItemDto[]> {
  const res = await invokeIpc<TodoItemDto[]>('delete_session_todo', {
    sessionId,
    todoId,
  });
  return res || [];
}

/**
 * Creates a new todo item in the session.
 */
export async function createSessionTodoIpc(
  sessionId: string,
  content: string,
  priority?: string
): Promise<TodoItemDto[]> {
  const res = await invokeIpc<TodoItemDto[]>('create_session_todo', {
    sessionId,
    content,
    priority,
  });
  return res || [];
}
