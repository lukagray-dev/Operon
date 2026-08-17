// Main Content Topbar local state

import type { GitDiffStats } from './types.js';

type TopbarChangeListener = () => void;

class TopbarStateManager {
  private title = 'New Session';
  private isProject = false;
  private projectName: string | null = null;
  private gitStats: GitDiffStats = {
    insertions: 0,
    deletions: 0,
    files_changed: 0,
    is_git_repo: false,
  };
  private isTerminalOpen = false;
  private isGitDiffOpen = false;
  private unfinishedTodoCount = 0;
  private totalTodoCount = 0;
  private listeners: Set<TopbarChangeListener> = new Set();

  public getUnfinishedTodoCount(): number {
    return this.unfinishedTodoCount;
  }

  public getTotalTodoCount(): number {
    return this.totalTodoCount;
  }

  public setTodoCounts(unfinished: number, total: number): void {
    if (this.unfinishedTodoCount !== unfinished || this.totalTodoCount !== total) {
      this.unfinishedTodoCount = unfinished;
      this.totalTodoCount = total;
      this.notify();
    }
  }

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

  public setProjectContext(isProject: boolean, name: string | null): void {
    this.isProject = isProject;
    this.projectName = name;
    this.notify();
  }

  public getGitStats(): GitDiffStats {
    return this.gitStats;
  }

  public setGitStats(stats: GitDiffStats): void {
    this.gitStats = stats;
    this.notify();
  }

  public getIsTerminalOpen(): boolean {
    return this.isTerminalOpen;
  }

  public toggleTerminal(): boolean {
    this.isTerminalOpen = !this.isTerminalOpen;
    this.notify();
    return this.isTerminalOpen;
  }

  public getIsGitDiffOpen(): boolean {
    return this.isGitDiffOpen;
  }

  public toggleGitDiff(): boolean {
    this.isGitDiffOpen = !this.isGitDiffOpen;
    this.notify();
    return this.isGitDiffOpen;
  }

  public subscribe(listener: TopbarChangeListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const topbarState = new TopbarStateManager();
