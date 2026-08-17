// ============================================================================
// Session Tasks / Todo Panel Reactive State Manager
//
// Hey friend! This state manager keeps track of the active session's todos,
// the current filter ('all' | 'pending' | 'completed'), and notifies any
// subscribed UI components whenever the tasks change.
// ============================================================================

import type { TodoFilter, TodoItemDto } from './types.js';

type StateListener = () => void;

class TodoPanelStateManager {
  /** List of todos loaded for the active session */
  private todos: TodoItemDto[] = [];

  /** Currently selected filter chip */
  private filter: TodoFilter = 'all';

  /** True while an IPC call is actively in flight */
  private isLoading = false;

  /** Observer pattern subscribers */
  private listeners: Set<StateListener> = new Set();

  /** Gets all loaded todos */
  public getTodos(): TodoItemDto[] {
    return this.todos;
  }

  /** Sets the list of todos and notifies subscribers */
  public setTodos(todos: TodoItemDto[]): void {
    this.todos = todos;
    this.notify();
  }

  /** Gets the active filter */
  public getFilter(): TodoFilter {
    return this.filter;
  }

  /** Sets the active filter and notifies subscribers */
  public setFilter(filter: TodoFilter): void {
    if (this.filter !== filter) {
      this.filter = filter;
      this.notify();
    }
  }

  /** Gets loading status */
  public getIsLoading(): boolean {
    return this.isLoading;
  }

  /** Sets loading status */
  public setIsLoading(loading: boolean): void {
    this.isLoading = loading;
  }

  /** Returns the count of unfinished (pending or in_progress) todos */
  public getUnfinishedCount(): number {
    return this.todos.filter((t) => t.status !== 'completed').length;
  }

  /** Returns the count of completed todos */
  public getCompletedCount(): number {
    return this.todos.filter((t) => t.status === 'completed').length;
  }

  /** Returns the list of todos matching the active filter */
  public getFilteredTodos(): TodoItemDto[] {
    switch (this.filter) {
      case 'pending':
        return this.todos.filter((t) => t.status !== 'completed');
      case 'completed':
        return this.todos.filter((t) => t.status === 'completed');
      case 'all':
      default:
        return this.todos;
    }
  }

  /** Subscribes a listener to state changes */
  public subscribe(listener: StateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Notifies all registered listeners */
  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch (err) {
        console.error('[TodoPanelState] Listener error:', err);
      }
    }
  }
}

export const todoPanelState = new TodoPanelStateManager();
