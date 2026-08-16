// ============================================================================
// Source Control & Git Diff Reactive State Manager
//
// Hey friend! This file manages all the runtime state for our right-sidebar
// Git panel. Just like how VS Code remembers which sections you folded, how
// tall you made each panel, what commit message you typed, and which repository
// you currently have selected in a multi-repo workspace, this singleton store
// keeps track of everything and notifies our UI so it can re-render smoothly.
// ============================================================================

import type {
  GitDiffDetails,
  GitGraphCommit,
  GitRepositoryInfo,
} from './types.js';

/** Callback listener type triggered on every state update */
type StateListener = () => void;

class RightSidebarStateManager {
  // --------------------------------------------------------------------------
  // Panel Visibility & Sizing
  // --------------------------------------------------------------------------
  /** Whether the right sidebar is currently open/visible on the screen */
  private isOpen = false;

  /** Overall pixel width of the right sidebar */
  private width = 340;

  /** Minimum permitted width in pixels when dragging the left resize handle */
  private minWidth = 260;

  /** Maximum permitted width in pixels when dragging the left resize handle */
  private maxWidth = 650;

  // --------------------------------------------------------------------------
  // Multi-Repository Context
  // --------------------------------------------------------------------------
  /** Path of the currently selected / active Git repository */
  private activeRepoPath: string | null = null;

  // --------------------------------------------------------------------------
  // Independent Section Heights (when resized with horizontal dividers)
  // --------------------------------------------------------------------------
  /** Height in pixels of the Repositories section (when not flex-filling) */
  private reposSectionHeight = 120;

  /** Height in pixels of the Changes section (when not flex-filling) */
  private changesSectionHeight = 260;

  /** Height in pixels of the Commit Graph section (when not flex-filling) */
  private graphSectionHeight = 240;

  // --------------------------------------------------------------------------
  // Git Data Payload (fetched from Tauri backend)
  // --------------------------------------------------------------------------
  /** High-level Git Diff details (branch, staged files, unstaged files, hunks) */
  private diffDetails: GitDiffDetails = {
    has_repo: false,
    repo_name: '',
    current_branch: 'main',
    total_insertions: 0,
    total_deletions: 0,
    unstaged_files: [],
    staged_files: [],
  };

  /** Discovered workspace repositories for multi-repo support */
  private repos: GitRepositoryInfo[] = [];

  /** Commit history timeline for the visual graph */
  private graphCommits: GitGraphCommit[] = [];

  /** Set of file paths whose inline unified diff hunk view is currently expanded */
  private expandedFiles: Set<string> = new Set();

  // --------------------------------------------------------------------------
  // Section Visibility Flags (Toggled via "..." header menu)
  // --------------------------------------------------------------------------
  /** Whether the REPOSITORIES section is visible in the panel */
  private reposVisible = true;

  /** Whether the CHANGES section is visible in the panel */
  private changesVisible = true;

  /** Whether the COMMIT GRAPH section is visible in the panel */
  private graphVisible = true;

  // --------------------------------------------------------------------------
  // Section Accordion Fold/Unfold States
  // --------------------------------------------------------------------------
  /** Whether the REPOSITORIES accordion body is expanded */
  private reposSectionExpanded = true;

  /** Whether the CHANGES main accordion body is expanded */
  private changesSectionExpanded = true;

  /** Whether the COMMIT GRAPH accordion body is expanded */
  private graphSectionExpanded = true;

  /** Whether the Staged Changes subgroup inside Changes is expanded */
  private stagedSectionExpanded = true;

  /** Whether the Changes (unstaged) subgroup inside Changes is expanded */
  private unstagedSectionExpanded = true;

  // --------------------------------------------------------------------------
  // Commit Input & Action Flags
  // --------------------------------------------------------------------------
  /** The current draft text typed into the commit message box */
  private commitMessage = '';

  /** True while calling AI LLM to generate a smart commit message */
  private isGeneratingMessage = false;

  /** True while Git commit command is actively running */
  private isCommitting = false;

  /** True while Git sync (pull/push) is actively running */
  private isSyncing = false;

  // --------------------------------------------------------------------------
  // Event Listeners (Observer Pattern)
  // --------------------------------------------------------------------------
  private listeners: Set<StateListener> = new Set();

  // ==========================================================================
  // Public Accessors & Mutators
  // ==========================================================================

  /** Checks if the sidebar is open */
  public getIsOpen(): boolean {
    return this.isOpen;
  }

  /** Explicitly opens or closes the sidebar */
  public setIsOpen(open: boolean): void {
    if (this.isOpen !== open) {
      this.isOpen = open;
      this.notify();
    }
  }

  /** Toggles open state between true and false */
  public toggleOpen(): boolean {
    this.isOpen = !this.isOpen;
    this.notify();
    return this.isOpen;
  }

  /** Gets current sidebar width */
  public getWidth(): number {
    return this.width;
  }

  /** Sets sidebar width safely clamped between min and max bounds */
  public setWidth(width: number): void {
    const clamped = Math.max(this.minWidth, Math.min(this.maxWidth, width));
    if (this.width !== clamped) {
      this.width = clamped;
      this.notify();
    }
  }

  /** Gets the active repository root path */
  public getActiveRepoPath(): string | null {
    return this.activeRepoPath;
  }

  /** Sets the active repository root path and notifies listeners */
  public setActiveRepoPath(path: string | null): void {
    if (this.activeRepoPath !== path) {
      this.activeRepoPath = path;
      this.notify();
    }
  }

  /** Gets Repositories section height */
  public getReposSectionHeight(): number {
    return this.reposSectionHeight;
  }

  /** Sets Repositories section height without triggering full re-render */
  public setReposSectionHeight(height: number, silent = false): void {
    const clamped = Math.max(50, Math.min(600, height));
    this.reposSectionHeight = clamped;
    if (!silent) this.notify();
  }

  /** Gets Changes section height */
  public getChangesSectionHeight(): number {
    return this.changesSectionHeight;
  }

  /** Sets Changes section height without triggering full re-render */
  public setChangesSectionHeight(height: number, silent = false): void {
    const clamped = Math.max(80, Math.min(800, height));
    this.changesSectionHeight = clamped;
    if (!silent) this.notify();
  }

  /** Gets Commit Graph section height */
  public getGraphSectionHeight(): number {
    return this.graphSectionHeight;
  }

  /** Sets Commit Graph section height */
  public setGraphSectionHeight(height: number, silent = false): void {
    const clamped = Math.max(80, Math.min(800, height));
    this.graphSectionHeight = clamped;
    if (!silent) this.notify();
  }

  /** Gets latest Git Diff details */
  public getDiffDetails(): GitDiffDetails {
    return this.diffDetails;
  }

  /** Updates Git Diff details and notifies subscribers */
  public setDiffDetails(details: GitDiffDetails): void {
    this.diffDetails = details;
    this.notify();
  }

  /** Gets discovered repositories */
  public getRepos(): GitRepositoryInfo[] {
    return this.repos;
  }

  /** Sets discovered repositories */
  public setRepos(repos: GitRepositoryInfo[]): void {
    this.repos = repos;
    this.notify();
  }

  /** Gets commit graph timeline */
  public getGraphCommits(): GitGraphCommit[] {
    return this.graphCommits;
  }

  /** Sets commit graph timeline */
  public setGraphCommits(commits: GitGraphCommit[]): void {
    this.graphCommits = commits;
    this.notify();
  }

  /** Checks if a specific file diff is expanded */
  public isFileExpanded(filePath: string): boolean {
    return this.expandedFiles.has(filePath);
  }

  /** Toggles expansion for an inline file diff */
  public toggleFileExpanded(filePath: string): void {
    if (this.expandedFiles.has(filePath)) {
      this.expandedFiles.delete(filePath);
    } else {
      this.expandedFiles.add(filePath);
    }
    this.notify();
  }

  /** Section Visibility Getters and Toggles */
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

  /** Accordion Section Expand / Collapse State */
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

  /** Commit message input state */
  public getCommitMessage(): string {
    return this.commitMessage;
  }

  public setCommitMessage(msg: string): void {
    this.commitMessage = msg;
    this.notify();
  }

  /** AI Generation & Process loading flags */
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

  /**
   * Subscribes a callback to receive notifications when state updates.
   * Returns an unsubscribe cleanup function.
   */
  public subscribe(listener: StateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Notifies all registered listeners of a change */
  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch (err) {
        console.error('[RightSidebarState] Error in listener:', err);
      }
    }
  }
}

/** Global singleton instance */
export const rightSidebarState = new RightSidebarStateManager();
