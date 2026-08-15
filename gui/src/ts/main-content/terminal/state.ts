// Terminal State Manager
//
// Manages reactive state for terminal tabs, active tab selection,
// panel expansion, and persisted viewport height.

import type { TerminalTab } from './types.js';

type TerminalChangeListener = () => void;

class TerminalStateManager {
  private tabs: TerminalTab[] = [];
  private activeTabId: string | null = null;
  private isPanelOpen = false;
  private nextTabNum = 1;
  private savedHeight = 280;
  private listeners: Set<TerminalChangeListener> = new Set();

  constructor() {
    const storedHeight = localStorage.getItem('operon-terminal-height');
    if (storedHeight) {
      const parsed = parseInt(storedHeight, 10);
      if (!isNaN(parsed) && parsed >= 120) {
        this.savedHeight = parsed;
      }
    }
  }

  public subscribe(listener: TerminalChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    this.listeners.forEach((listener) => {
      try {
        listener();
      } catch (err) {
        console.error('[TerminalState] Error in listener callback:', err);
      }
    });
  }

  public getTabs(): TerminalTab[] {
    return this.tabs;
  }

  public getActiveTabId(): string | null {
    return this.activeTabId;
  }

  public getActiveTab(): TerminalTab | undefined {
    return this.tabs.find((t) => t.id === this.activeTabId);
  }

  public setActiveTabId(id: string | null): void {
    if (this.activeTabId !== id) {
      this.activeTabId = id;
      this.notify();
    }
  }

  public addTab(tab: TerminalTab): void {
    this.tabs.push(tab);
    this.activeTabId = tab.id;
    this.notify();
  }

  public removeTab(id: string): TerminalTab | undefined {
    const idx = this.tabs.findIndex((t) => t.id === id);
    if (idx === -1) return undefined;

    const [removed] = this.tabs.splice(idx, 1);

    if (this.activeTabId === id) {
      if (this.tabs.length > 0) {
        const nextIdx = Math.min(idx, this.tabs.length - 1);
        this.activeTabId = this.tabs[nextIdx].id;
      } else {
        this.activeTabId = null;
        this.nextTabNum = 1;
      }
    }

    if (this.tabs.length === 0) {
      this.nextTabNum = 1;
    }

    this.notify();
    return removed;
  }

  public getNextTabName(): string {
    return `pwsh ${this.nextTabNum++}`;
  }

  public isOpen(): boolean {
    return this.isPanelOpen;
  }

  public setOpen(open: boolean): void {
    if (this.isPanelOpen !== open) {
      this.isPanelOpen = open;
      this.notify();
    }
  }

  public getSavedHeight(): number {
    return this.savedHeight;
  }

  public setSavedHeight(height: number): void {
    const clamped = Math.max(120, height);
    this.savedHeight = clamped;
    localStorage.setItem('operon-terminal-height', String(clamped));
  }
}

export const terminalState = new TerminalStateManager();
