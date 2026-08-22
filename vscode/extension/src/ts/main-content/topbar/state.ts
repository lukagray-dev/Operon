// ============================================================================
// Main Content Topbar Reactive State Manager for VS Code
// ============================================================================

type TopbarListener = () => void;

class TopbarStateManager {
  private title = 'New Session';
  private isProject = false;
  private projectName: string | null = null;
  private unfinishedTodoCount = 0;
  private totalTodoCount = 0;
  private listeners: Set<TopbarListener> = new Set();

  public getTitle(): string {
    return this.title;
  }

  public setTitle(title: string): void {
    if (this.title !== title) {
      this.title = title;
      this.notify();
    }
  }

  public getIsProject(): boolean {
    return this.isProject;
  }

  public getProjectName(): string | null {
    return this.projectName;
  }

  public setProjectContext(isProject: boolean, projectName: string | null): void {
    if (this.isProject !== isProject || this.projectName !== projectName) {
      this.isProject = isProject;
      this.projectName = projectName;
      this.notify();
    }
  }

  public getTodoCounts(): { unfinished: number; total: number } {
    return {
      unfinished: this.unfinishedTodoCount,
      total: this.totalTodoCount,
    };
  }

  public setTodoCounts(unfinished: number, total: number): void {
    if (this.unfinishedTodoCount !== unfinished || this.totalTodoCount !== total) {
      this.unfinishedTodoCount = unfinished;
      this.totalTodoCount = total;
      this.notify();
    }
  }

  public subscribe(listener: TopbarListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch (err) {
        console.error('[TopbarState] Listener error:', err);
      }
    }
  }
}

export const topbarState = new TopbarStateManager();
