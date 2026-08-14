// Empty State Local State Manager

type EmptyStateListener = () => void;

class EmptyStateManager {
  private visible = true;
  private listeners: Set<EmptyStateListener> = new Set();

  public isVisible(): boolean {
    return this.visible;
  }

  public setVisible(visible: boolean): void {
    if (this.visible !== visible) {
      this.visible = visible;
      this.notify();
    }
  }

  public subscribe(listener: EmptyStateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const emptyState = new EmptyStateManager();
