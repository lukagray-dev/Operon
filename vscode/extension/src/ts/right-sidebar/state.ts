// ============================================================================
// Session Tasks / Todo Panel Reactive State Manager for VS Code
// ============================================================================

import type { TodoFilter, TodoItemDto } from './types.js';

type StateListener = () => void;

class TodoPanelStateManager {
  private todos: TodoItemDto[] = [];
  private filter: TodoFilter = 'all';
  private isLoading = false;
  private isOpen = false;
  private listeners: Set<StateListener> = new Set();

  public getTodos(): TodoItemDto[] {
    return this.todos;
  }

  public setTodos(todos: TodoItemDto[]): void {
    this.todos = todos;
    this.notify();
  }

  public getFilter(): TodoFilter {
    return this.filter;
  }

  public setFilter(filter: TodoFilter): void {
    if (this.filter !== filter) {
      this.filter = filter;
      this.notify();
    }
  }

  public getIsLoading(): boolean {
    return this.isLoading;
  }

  public setIsLoading(loading: boolean): void {
    this.isLoading = loading;
  }

  public getIsOpen(): boolean {
    return this.isOpen;
  }

  public setIsOpen(open: boolean): void {
    if (this.isOpen !== open) {
      this.isOpen = open;
      this.notify();
    }
  }

  public toggle(): void {
    this.setIsOpen(!this.isOpen);
  }

  public getUnfinishedCount(): number {
    return this.todos.filter((t) => t.status !== 'completed').length;
  }

  public getCompletedCount(): number {
    return this.todos.filter((t) => t.status === 'completed').length;
  }

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

  public subscribe(listener: StateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

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
