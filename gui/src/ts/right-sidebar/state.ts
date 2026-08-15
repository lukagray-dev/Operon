// Source Control & Git Diff Reactive State Manager

import type {
  GitDiffDetails,
  GitGraphCommit,
  GitRepositoryInfo,
} from './types.js';

type StateListener = () => void;

class RightSidebarStateManager {
  private isOpen = false;
  private width = 340;
  private minWidth = 260;
  private maxWidth = 650;

  private diffDetails: GitDiffDetails = {
    has_repo: false,
    repo_name: '',
    current_branch: 'main',
    total_insertions: 0,
    total_deletions: 0,
    unstaged_files: [],
    staged_files: [],
  };

  private repos: GitRepositoryInfo[] = [];
  private graphCommits: GitGraphCommit[] = [];
  private expandedFiles: Set<string> = new Set();

  private reposVisible = true;
  private changesVisible = true;
  private graphVisible = true;

  private reposSectionExpanded = true;
  private changesSectionExpanded = true;
  private graphSectionExpanded = true;
  private stagedSectionExpanded = true;
  private unstagedSectionExpanded = true;

  private commitMessage = '';
  private isGeneratingMessage = false;
  private isCommitting = false;
  private isSyncing = false;

  private listeners: Set<StateListener> = new Set();

  public getIsOpen(): boolean {
    return this.isOpen;
  }

  public setIsOpen(open: boolean): void {
    if (this.isOpen !== open) {
      this.isOpen = open;
      this.notify();
    }
  }

  public toggleOpen(): boolean {
    this.isOpen = !this.isOpen;
    this.notify();
    return this.isOpen;
  }

  public getWidth(): number {
    return this.width;
  }

  public setWidth(width: number): void {
    const clamped = Math.max(this.minWidth, Math.min(this.maxWidth, width));
    if (this.width !== clamped) {
      this.width = clamped;
      this.notify();
    }
  }

  public getDiffDetails(): GitDiffDetails {
    return this.diffDetails;
  }

  public setDiffDetails(details: GitDiffDetails): void {
    this.diffDetails = details;
    this.notify();
  }

  public getRepos(): GitRepositoryInfo[] {
    return this.repos;
  }

  public setRepos(repos: GitRepositoryInfo[]): void {
    this.repos = repos;
    this.notify();
  }

  public getGraphCommits(): GitGraphCommit[] {
    return this.graphCommits;
  }

  public setGraphCommits(commits: GitGraphCommit[]): void {
    this.graphCommits = commits;
    this.notify();
  }

  public isFileExpanded(filePath: string): boolean {
    return this.expandedFiles.has(filePath);
  }

  public toggleFileExpanded(filePath: string): void {
    if (this.expandedFiles.has(filePath)) {
      this.expandedFiles.delete(filePath);
    } else {
      this.expandedFiles.add(filePath);
    }
    this.notify();
  }

  public getReposVisible(): boolean {
    return this.reposVisible;
  }

  public toggleReposVisible(): void {
    this.reposVisible = !this.reposVisible;
    this.notify();
  }

  public getChangesVisible(): boolean {
    return this.changesVisible;
  }

  public toggleChangesVisible(): void {
    this.changesVisible = !this.changesVisible;
    this.notify();
  }

  public getGraphVisible(): boolean {
    return this.graphVisible;
  }

  public toggleGraphVisible(): void {
    this.graphVisible = !this.graphVisible;
    this.notify();
  }

  public isReposSectionExpanded(): boolean {
    return this.reposSectionExpanded;
  }

  public toggleReposSection(): void {
    this.reposSectionExpanded = !this.reposSectionExpanded;
    this.notify();
  }

  public isChangesSectionExpanded(): boolean {
    return this.changesSectionExpanded;
  }

  public toggleChangesSection(): void {
    this.changesSectionExpanded = !this.changesSectionExpanded;
    this.notify();
  }

  public isGraphSectionExpanded(): boolean {
    return this.graphSectionExpanded;
  }

  public toggleGraphSection(): void {
    this.graphSectionExpanded = !this.graphSectionExpanded;
    this.notify();
  }

  public isStagedSectionExpanded(): boolean {
    return this.stagedSectionExpanded;
  }

  public toggleStagedSection(): void {
    this.stagedSectionExpanded = !this.stagedSectionExpanded;
    this.notify();
  }

  public isUnstagedSectionExpanded(): boolean {
    return this.unstagedSectionExpanded;
  }

  public toggleUnstagedSection(): void {
    this.unstagedSectionExpanded = !this.unstagedSectionExpanded;
    this.notify();
  }

  public getCommitMessage(): string {
    return this.commitMessage;
  }

  public setCommitMessage(msg: string): void {
    this.commitMessage = msg;
    this.notify();
  }

  public getIsGeneratingMessage(): boolean {
    return this.isGeneratingMessage;
  }

  public setIsGeneratingMessage(generating: boolean): void {
    this.isGeneratingMessage = generating;
    this.notify();
  }

  public getIsCommitting(): boolean {
    return this.isCommitting;
  }

  public setIsCommitting(committing: boolean): void {
    this.isCommitting = committing;
    this.notify();
  }

  public getIsSyncing(): boolean {
    return this.isSyncing;
  }

  public setIsSyncing(syncing: boolean): void {
    this.isSyncing = syncing;
    this.notify();
  }

  public subscribe(listener: StateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}

export const rightSidebarState = new RightSidebarStateManager();
