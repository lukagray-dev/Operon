/**
 * Right Sidebar Component Controller
 * 
 * Hey friend! This module implements the frontend controller for our right-sidebar
 * Git Diff Preview panel. It manages the resizability, file tree rendering, Stage/Unstage
 * events, file revertions, and unified hunk-level diff renderings.
 * 
 * Features:
 * - Resizable panel with horizontal drag handlers.
 * - Dynamic file lists for both staged and unstaged changes.
 * - Hunk rendering with red/green line coloring and line numbers.
 * - Integrates stage, unstage, and revert commands.
 */

'use strict';

import * as IPC from '../shared/ipc.js';
import { showError, showSuccess } from '../shared/toast.js';

class RightSidebarController {
    constructor() {
        // Current state variables
        this.isOpen = false;
        this.isResizing = false;
        this.savedWidth = parseInt(localStorage.getItem('operon-right-sidebar-width')) || 340;
        
        // Cache list of expanded file paths to preserve expand state during refreshes
        this.expandedFiles = new Set();
        
        // DOM element references
        this.sidebarEl = null;
        this.toggleBtnEl = null;
        this.statsBtnContainerEl = null;
        this.closeBtnEl = null;
        this.resizeHandleEl = null;
        
        // Section accordions
        this.unstagedHeaderEl = null;
        this.unstagedContentEl = null;
        this.unstagedCountEl = null;
        this.unstagedStatsEl = null;
        
        this.stagedHeaderEl = null;
        this.stagedContentEl = null;
        this.stagedCountEl = null;
        this.stagedStatsEl = null;
        
        // Bulk actions
        this.stageAllBtnEl = null;
        this.revertAllBtnEl = null;
        
        // Auto-initialize
        this.init();
    }

    /**
     * Locate DOM elements and attach listeners
     */
    async init() {
        try {
            // Retrieve UI elements
            this.sidebarEl = document.getElementById('right-sidebar');
            this.toggleBtnEl = document.getElementById('git-diff-toggle-btn');
            this.statsBtnContainerEl = document.getElementById('git-diff-btn-stats');
            this.closeBtnEl = document.getElementById('right-sidebar-close-btn');
            this.resizeHandleEl = document.getElementById('right-sidebar-resize-handle');
            
            // Accordions
            this.unstagedHeaderEl = document.getElementById('git-header-unstaged');
            this.unstagedContentEl = document.getElementById('git-content-unstaged');
            this.unstagedCountEl = document.getElementById('git-count-unstaged');
            this.unstagedStatsEl = document.getElementById('git-stats-unstaged');
            
            this.stagedHeaderEl = document.getElementById('git-header-staged');
            this.stagedContentEl = document.getElementById('git-content-staged');
            this.stagedCountEl = document.getElementById('git-count-staged');
            this.stagedStatsEl = document.getElementById('git-stats-staged');
            
            // Bulk buttons
            this.stageAllBtnEl = document.getElementById('git-btn-stage-all');
            this.revertAllBtnEl = document.getElementById('git-btn-revert-all');
            
            if (!this.sidebarEl || !this.toggleBtnEl) {
                console.warn('Right sidebar DOM elements not found. Skipping initialization.');
                return;
            }

            // Register toggle click
            this.toggleBtnEl.addEventListener('click', () => this.toggleSidebar());
            if (this.closeBtnEl) {
                this.closeBtnEl.addEventListener('click', () => this.closeSidebar());
            }
            
            // Register resizing drag listeners
            if (this.resizeHandleEl) {
                this.resizeHandleEl.addEventListener('mousedown', (e) => this.startResize(e));
            }
            
            // Accordion toggle click handlers
            if (this.unstagedHeaderEl) {
                this.unstagedHeaderEl.addEventListener('click', (e) => {
                    // Prevent toggle when clicking action headers if they exist
                    if (e.target.closest('.right-sidebar__action-btn-header')) return;
                    const section = document.getElementById('git-section-unstaged');
                    section.classList.toggle('collapsed');
                });
            }
            
            if (this.stagedHeaderEl) {
                this.stagedHeaderEl.addEventListener('click', (e) => {
                    if (e.target.closest('.right-sidebar__action-btn-header')) return;
                    const section = document.getElementById('git-section-staged');
                    section.classList.toggle('collapsed');
                });
            }
            
            // Bulk action clicks
            if (this.stageAllBtnEl) {
                this.stageAllBtnEl.addEventListener('click', () => this.stageAllChanges());
            }
            if (this.revertAllBtnEl) {
                this.revertAllBtnEl.addEventListener('click', () => this.revertAllChanges());
            }

            // Perform initial quick stats check
            await this.refreshQuickStats();
            
            console.log('Right sidebar controller initialized successfully');
        } catch (error) {
            console.error('Failed to initialize right sidebar controller:', error);
        }
    }

    /**
     * Start the horizontal dragging action to resize the right sidebar width
     */
    startResize(e) {
        e.preventDefault();
        this.isResizing = true;
        this.sidebarEl.classList.add('resizing');
        document.body.classList.add('right-sidebar-resizing');
        
        const onMouseMove = (moveEvent) => {
            if (!this.isResizing) return;
            
            // Calculate width based on position from the right side of the screen
            let newWidth = window.innerWidth - moveEvent.clientX;
            
            // Apply minimum and maximum constraints (use the smaller of CSS max-width and 60% of viewport)
            const minWidth = parseInt(getComputedStyle(document.documentElement).getPropertyValue('--right-sidebar-min-width')) || 240;
            const cssMaxWidth = parseInt(getComputedStyle(document.documentElement).getPropertyValue('--right-sidebar-max-width')) || 650;
            const maxWidth = Math.min(cssMaxWidth, window.innerWidth * 0.6);
            
            if (newWidth < minWidth) newWidth = minWidth;
            if (newWidth > maxWidth) newWidth = maxWidth;
            
            this.savedWidth = newWidth;
            localStorage.setItem('operon-right-sidebar-width', newWidth);
            
            // Apply variables dynamically
            document.documentElement.style.setProperty('--right-sidebar-width', `${newWidth}px`);
        };
        
        const onMouseUp = () => {
            this.isResizing = false;
            if (this.sidebarEl) {
                this.sidebarEl.classList.remove('resizing');
            }
            document.body.classList.remove('right-sidebar-resizing');
            window.removeEventListener('mousemove', onMouseMove);
            window.removeEventListener('mouseup', onMouseUp);
        };
        
        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
    }

    /**
     * Toggle right-sidebar visibility panel
     */
    async toggleSidebar() {
        if (this.isOpen) {
            this.closeSidebar();
        } else {
            await this.openSidebar();
        }
    }

    /**
     * Slide open sidebar panel and fetch repository details
     */
    async openSidebar() {
        this.isOpen = true;
        this.sidebarEl.classList.remove('collapsed');
        this.toggleBtnEl.classList.add('active');
        
        // Apply custom width
        document.documentElement.style.setProperty('--right-sidebar-width', `${this.savedWidth}px`);
        
        // Populate changes list
        await this.refreshDetails();
    }

    /**
     * Collapse right-sidebar panel and clear states
     */
    closeSidebar() {
        this.isOpen = false;
        if (this.sidebarEl) {
            this.sidebarEl.classList.add('collapsed');
        }
        if (this.toggleBtnEl) {
            this.toggleBtnEl.classList.remove('active');
        }
        // Reset sidebar width variable so input panel slides back to right edge
        document.documentElement.style.setProperty('--right-sidebar-width', '0px');
    }

    /**
     * Query quick stats and display changes badge inside top right bar button
     */
    async refreshQuickStats() {
        try {
            if (!IPC.isTauriAvailable()) return;
            
            // Fetch current project workspace directory
            const projectDir = window.sessionManager?.currentProjectDir || null;
            const stats = await IPC.getGitDiffStats(projectDir);
            
            if (stats.hasRepo && (stats.insertions > 0 || stats.deletions > 0)) {
                // Populate stat text
                const addedSpan = this.statsBtnContainerEl.querySelector('.git-diff-btn-stats__added');
                const deletedSpan = this.statsBtnContainerEl.querySelector('.git-diff-btn-stats__deleted');
                
                if (addedSpan) addedSpan.textContent = `+${stats.insertions}`;
                if (deletedSpan) deletedSpan.textContent = `-${stats.deletions}`;
                
                this.statsBtnContainerEl.style.display = 'flex';
            } else {
                this.statsBtnContainerEl.style.display = 'none';
            }
        } catch (error) {
            console.error('Failed to query quick git stats:', error);
        }
    }

    /**
     * Fetch detailed file differences lists and render them in the panel
     */
    async refreshDetails() {
        try {
            if (!IPC.isTauriAvailable()) return;
            
            const projectDir = window.sessionManager?.currentProjectDir || null;
            const details = await IPC.getGitDiffDetails(projectDir);
            
            // If repository is missing, show empty placeholder states
            if (!details.hasRepo) {
                this.renderEmptyState("No git repository found in workspace.");
                return;
            }
            
            // Render accordion segments
            this.renderSection(this.unstagedContentEl, this.unstagedCountEl, this.unstagedStatsEl, details.unstagedFiles, false);
            this.renderSection(this.stagedContentEl, this.stagedCountEl, this.stagedStatsEl, details.stagedFiles, true);
            
            // Update quick button stats alongside panel details
            await this.refreshQuickStats();
        } catch (error) {
            console.error('Failed to refresh git panel details:', error);
            showError(`Git Diff error: ${error}`);
        }
    }

    /**
     * Render empty placeholders when no git changes exist
     */
    renderEmptyState(message) {
        if (this.unstagedContentEl) {
            this.unstagedContentEl.innerHTML = `<div style="padding: 16px; font-size:12px; color:#6e6e6e; text-align:center;">${message}</div>`;
        }
        if (this.stagedContentEl) {
            this.stagedContentEl.innerHTML = '';
        }
        if (this.unstagedCountEl) this.unstagedCountEl.textContent = '0';
        if (this.stagedCountEl) this.stagedCountEl.textContent = '0';
        if (this.unstagedStatsEl) this.unstagedStatsEl.innerHTML = '';
        if (this.stagedStatsEl) this.stagedStatsEl.innerHTML = '';
    }

    /**
     * Helper to render either the staged or unstaged accordion file listing
     */
    renderSection(contentEl, countEl, statsEl, files, isStaged) {
        if (!contentEl) return;
        
        contentEl.innerHTML = '';
        if (countEl) countEl.textContent = files.length.toString();
        
        // Aggregate statistics for the accordion header
        let totalIns = 0;
        let totalDel = 0;
        files.forEach(f => {
            totalIns += f.insertions;
            totalDel += f.deletions;
        });
        
        if (statsEl) {
            let statsHtml = '';
            if (totalIns > 0) statsHtml += `<span style="color:var(--color-diff-added-border); margin-right:4px;">+${totalIns}</span>`;
            if (totalDel > 0) statsHtml += `<span style="color:var(--color-diff-removed-border);">${totalDel}</span>`;
            statsEl.innerHTML = statsHtml;
        }
        
        if (files.length === 0) {
            contentEl.innerHTML = `<div style="padding: 12px 16px; font-size: 11px; color: #6e6e6e;">No changes</div>`;
            return;
        }
        
        files.forEach(file => {
            const isExpanded = this.expandedFiles.has(file.path);
            
            const fileItem = document.createElement('div');
            fileItem.className = `right-sidebar__file-item ${isExpanded ? 'expanded' : ''}`;
            fileItem.setAttribute('data-file-path', file.path);
            
            // Build status indicator class
            let statusTagClass = 'right-sidebar__file-status-tag--modified';
            if (file.status === 'added') statusTagClass = 'right-sidebar__file-status-tag--added';
            else if (file.status === 'deleted') statusTagClass = 'right-sidebar__file-status-tag--deleted';
            else if (file.status === 'untracked') statusTagClass = 'right-sidebar__file-status-tag--untracked';
            
            // Stage/Unstage button SVG selection
            const actionIcon = isStaged 
                ? './assets/icons/action/close.svg' // unstage action uses close/remove icon
                : './assets/icons/action/add.svg';   // stage action uses add/stage icon
            
            const actionTitle = isStaged ? 'Unstage file' : 'Stage file';
            
            // Compile unified diff hunk lines
            let diffViewerHtml = '';
            if (file.hunks && file.hunks.length > 0) {
                file.hunks.forEach(hunk => {
                    diffViewerHtml += `<div class="right-sidebar__diff-hunk">`;
                    diffViewerHtml += `<div class="right-sidebar__diff-hunk-header">${this.escapeHtml(hunk.header)}</div>`;
                    
                    hunk.lines.forEach(line => {
                        let lineClass = 'right-sidebar__diff-line';
                        if (line.lineType === '+') lineClass += ' right-sidebar__diff-line--added';
                        else if (line.lineType === '-') lineClass += ' right-sidebar__diff-line--removed';
                        
                        const oldNum = line.oldLineNum !== null ? line.oldLineNum : '';
                        const newNum = line.newLineNum !== null ? line.newLineNum : '';
                        
                        diffViewerHtml += `
                            <div class="${lineClass}">
                                <div class="right-sidebar__diff-line-nums">
                                    <span>${oldNum}</span>
                                    <span>${newNum}</span>
                                </div>
                                <div class="right-sidebar__diff-line-content">${this.escapeHtml(line.content)}</div>
                            </div>
                        `;
                    });
                    diffViewerHtml += `</div>`;
                });
            } else {
                diffViewerHtml = `<div style="padding:12px; font-size:11px; color:#6e6e6e; text-align:center;">Binary file diff not supported</div>`;
            }
            
            fileItem.innerHTML = `
                <div class="right-sidebar__file-row">
                    <img class="right-sidebar__file-chevron" src="./assets/icons/sidebar/chevron-down.svg" alt="chevron" />
                    <span class="right-sidebar__file-name" title="${this.escapeHtml(file.path)}">${this.escapeHtml(file.path.split(/[/\\]/).pop())}</span>
                    
                    <div class="right-sidebar__file-actions">
                        <!-- Stats Badge -->
                        <span class="right-sidebar__file-stats">
                            <span style="color:var(--color-diff-added-border)">+${file.insertions}</span>
                            <span style="color:var(--color-diff-removed-border)">-${file.deletions}</span>
                        </span>
                        
                        <!-- Status Type Label -->
                        <span class="right-sidebar__file-status-tag ${statusTagClass}">${file.status}</span>
                        
                        <!-- Revert button (only visible on unstaged items) -->
                        ${!isStaged ? `
                        <button class="right-sidebar__file-btn right-sidebar__file-btn--revert" title="Revert modifications">
                            <img src="./assets/icons/sidebar/delete.svg" alt="revert" />
                        </button>
                        ` : ''}
                        
                        <!-- Stage/Unstage button -->
                        <button class="right-sidebar__file-btn right-sidebar__file-btn--action" title="${actionTitle}">
                            <img src="${actionIcon}" alt="action" />
                        </button>
                    </div>
                </div>
                <div class="right-sidebar__file-diff">
                    ${diffViewerHtml}
                </div>
            `;
            
            // Toggle file diff visibility collapse
            const row = fileItem.querySelector('.right-sidebar__file-row');
            row.addEventListener('click', (e) => {
                if (e.target.closest('.right-sidebar__file-btn')) return;
                
                const isCurrentlyExpanded = fileItem.classList.contains('expanded');
                if (isCurrentlyExpanded) {
                    fileItem.classList.remove('expanded');
                    this.expandedFiles.delete(file.path);
                } else {
                    fileItem.classList.add('expanded');
                    this.expandedFiles.add(file.path);
                }
            });
            
            // Bind single file stage/unstage command
            const actionBtn = fileItem.querySelector('.right-sidebar__file-btn--action');
            if (actionBtn) {
                actionBtn.addEventListener('click', async (e) => {
                    e.stopPropagation();
                    const projectDir = window.sessionManager?.currentProjectDir || null;
                    try {
                        if (isStaged) {
                            await IPC.unstageGitFile(projectDir, file.path);
                        } else {
                            await IPC.stageGitFile(projectDir, file.path);
                        }
                        await this.refreshDetails();
                    } catch (err) {
                        showError(`Git action failed: ${err}`);
                    }
                });
            }
            
            // Bind single file revert command
            const revertBtn = fileItem.querySelector('.right-sidebar__file-btn--revert');
            if (revertBtn) {
                revertBtn.addEventListener('click', async (e) => {
                    e.stopPropagation();
                    // Prompt user before discarding modifications destructively
                    const confirmed = confirm(`Are you sure you want to discard all changes in ${file.path.split(/[/\\]/).pop()}? This cannot be undone.`);
                    if (!confirmed) return;
                    
                    const projectDir = window.sessionManager?.currentProjectDir || null;
                    try {
                        await IPC.revertGitFile(projectDir, file.path);
                        showSuccess(`Discarded changes in ${file.path.split(/[/\\]/).pop()}`);
                        
                        // Clean expansion cache since file modifications disappear
                        this.expandedFiles.delete(file.path);
                        await this.refreshDetails();
                    } catch (err) {
                        showError(`Git discard changes failed: ${err}`);
                    }
                });
            }
            
            contentEl.appendChild(fileItem);
        });
    }

    /**
     * Stage all unstaged edits inside repository
     */
    async stageAllChanges() {
        const projectDir = window.sessionManager?.currentProjectDir || null;
        try {
            await IPC.stageAllGitFiles(projectDir);
            showSuccess("Staged all modifications.");
            await this.refreshDetails();
        } catch (error) {
            showError(`Stage all changes failed: ${error}`);
        }
    }

    /**
     * Discard all unstaged modifications inside repository
     */
    async revertAllChanges() {
        const confirmed = confirm("Are you sure you want to discard all unstaged changes in the repository? This cannot be undone.");
        if (!confirmed) return;
        
        const projectDir = window.sessionManager?.currentProjectDir || null;
        try {
            await IPC.revertAllGitFiles(projectDir);
            showSuccess("Discarded all modifications.");
            this.expandedFiles.clear(); // Clear expanded files list cache
            await this.refreshDetails();
        } catch (error) {
            showError(`Revert all changes failed: ${error}`);
        }
    }

    /**
     * Escape HTML helper
     */
    escapeHtml(raw) {
        if (raw === null || raw === undefined) return '';
        return String(raw)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#039;');
    }
}

// Auto-initialize when DOM is ready
let rightSidebarController = null;

if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => {
        rightSidebarController = new RightSidebarController();
        window.rightSidebarController = rightSidebarController;
    });
} else {
    rightSidebarController = new RightSidebarController();
    window.rightSidebarController = rightSidebarController;
}

export default RightSidebarController;
