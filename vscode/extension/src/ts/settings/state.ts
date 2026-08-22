// Settings Window Reactive State Manager

import type { SettingsTabId } from './sidebar/types.js';

type SettingsStateListener = () => void;

class SettingsStateManager {
  private activeTab: SettingsTabId = 'general';
  private listeners: Set<SettingsStateListener> = new Set();

  public getActiveTab(): SettingsTabId {
    return this.activeTab;
  }

  public setActiveTab(tab: SettingsTabId): void {
    if (this.activeTab !== tab) {
      this.activeTab = tab;
      this.notify();
    }
  }

  public subscribe(listener: SettingsStateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const settingsState = new SettingsStateManager();
