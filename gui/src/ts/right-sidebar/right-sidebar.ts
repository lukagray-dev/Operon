// ============================================================================
// Source Control & Git Diff Right Sidebar Controller & Renderer
//
// Hey friend! This is the main controller and DOM renderer for our VS Code-style
// Source Control & Git Diff right sidebar.
//
// Here is how everything works under the hood:
// 1. Header: Uppercase "SOURCE CONTROL" title + overflow "..." menu button.
// 2. Multi-Repo Switching: Supports workspace multi-repositories. Clicking any
//    repository immediately loads its active branch, changed files, and commit graph.
// 3. Section Flex Layout: Sections automatically distribute vertical space.
//    The bottom-most expanded section (like the Commit Graph) automatically
//    flex-fills all remaining height down to the bottom of the window so there is
//    no dead empty space.
// 4. Smooth Resizing: Horizontal dividers allow you to drag and resize sections
//    smoothly at 60fps without flickering or DOM destruction.
// 5. Context Menus: Floating VS Code style popups with nested hover submenus.
// ============================================================================

import { sidebarState } from '../left-sidebar/state.js';
import * as menuDefs from './menu-data.js';
import {
  getGitCommitGraphIpc,
  getGitDiffDetailsIpc,
  getWorkspaceRepositoriesIpc,
  gitCommitChangesIpc,
  gitFetchChangesIpc,
  gitGenerateCommitMessageIpc,
  gitPullChangesIpc,
  gitPushChangesIpc,
  gitRevertAllFilesIpc,
  gitRevertFileIpc,
  gitStageAllFilesIpc,
  gitStageFileIpc,
  gitUnstageAllFilesIpc,
  gitUnstageFileIpc,
} from './ipc.js';
import { rightSidebarState } from './state.js';
import { refreshTodoPanel, renderTodoPanel } from './todo-panel/todo-panel.js';
import type { ContextMenuItem, GitFileDiff, GitRepositoryInfo } from './types.js';

// ----------------------------------------------------------------------------
// Local state for Context Menu positioning and submenus
// ----------------------------------------------------------------------------
let activeMenuId: string | null = null;
let activeSubmenuItems: ContextMenuItem[] | null = null;
let menuCoords = { x: 0, y: 0 };
let submenuY = 0;

/**
 * Returns the currently active workspace / repository directory path.
 */
function resolveCurrentRepoPath(): string | undefined {
  return (
    rightSidebarState.getActiveRepoPath() ||
    sidebarState.getActiveProjectPath() ||
    undefined
  );
}

/**
 * Initializes the Right Sidebar component, binds buttons and shortcuts.
 */
export function initRightSidebar(): void {
  // 1. Topbar git-diff toggle button click listener
  const topbarBtn = document.getElementById('btn-topbar-git-commit');
  if (topbarBtn) {
    topbarBtn.addEventListener('click', () => {
      rightSidebarState.openPanel('git');
    });
  }

  // 2. Titlebar "View -> Toggle git-diff panel" menu item listener
  const menuItemToggle = document.getElementById('menu-item-toggle-git-diff');
  if (menuItemToggle) {
    menuItemToggle.addEventListener('click', () => {
      rightSidebarState.openPanel('git');
    });
  }

  // 3. Global keyboard shortcut Ctrl+G (or Cmd+G on macOS)
  window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'g') {
      e.preventDefault();
      rightSidebarState.openPanel('git');
    }
  });

  // 4. Re-render whenever our state updates
  rightSidebarState.subscribe(() => {
    renderRightSidebar();
  });

  // 5. When the user switches projects/sessions in the left sidebar, reset active repo and refresh
  sidebarState.subscribe(() => {
    rightSidebarState.setActiveRepoPath(null);
    refreshRightSidebar();
    refreshTodoPanel();
  });

  // 6. Initial data fetch on application startup
  refreshRightSidebar();
  refreshTodoPanel();
}

/**
 * Fetches fresh Git status, changed files, and commit graph from our backend.
 * If targetRepoPath is specified, switches the active repo context to that path.
 */
export async function refreshRightSidebar(targetRepoPath?: string): Promise<void> {
  if (targetRepoPath) {
    rightSidebarState.setActiveRepoPath(targetRepoPath);
  }

  const workspaceRoot = sidebarState.getActiveProjectPath() || undefined;
  const currentRepoPath = rightSidebarState.getActiveRepoPath() || workspaceRoot;

  try {
    // 1. Fetch details (staged/unstaged files) for the active repo
    const details = await getGitDiffDetailsIpc(currentRepoPath);
    rightSidebarState.setDiffDetails(details);

    // 2. Discover all workspace repos (from root workspace project)
    const rawRepos = await getWorkspaceRepositoriesIpc(workspaceRoot);
    if (rawRepos.length > 0) {
      // Determine which repo should be marked active
      const activePath = currentRepoPath;
      const repos: GitRepositoryInfo[] = rawRepos.map((r) => {
        const isSelected = activePath
          ? r.path === activePath || r.name === activePath
          : r.is_active;
        return {
          ...r,
          is_active: isSelected,
        };
      });

      // If none was matched as active, mark the first one as active
      if (!repos.some((r) => r.is_active) && repos.length > 0) {
        repos[0].is_active = true;
        rightSidebarState.setActiveRepoPath(repos[0].path);
      }

      rightSidebarState.setRepos(repos);
    } else {
      rightSidebarState.setRepos([]);
    }

    // 3. Fetch commit graph history for the active repo
    if (details.has_repo || currentRepoPath) {
      const graph = await getGitCommitGraphIpc(currentRepoPath, 50, 0);
      rightSidebarState.setGraphCommits(graph);
    } else {
      rightSidebarState.setGraphCommits([]);
    }
  } catch (err) {
    console.warn('[RightSidebar] Failed to refresh Git workspace data:', err);
  }
}

/**
 * Master DOM rendering function. Constructs the sidebar layout structure.
 */
export function renderRightSidebar(): void {
  const aside = document.getElementById('right-sidebar');
  if (!aside) return;

  const isOpen = rightSidebarState.getIsOpen();
  const width = rightSidebarState.getWidth();

  // If panel is closed, hide and clean up
  if (!isOpen) {
    aside.style.display = 'none';
    aside.classList.remove('open');
    closeAllMenus();
    return;
  }

  // Reveal panel and set current width
  aside.style.display = 'flex';
  aside.style.width = `${width}px`;
  aside.classList.add('open');
  aside.innerHTML = '';

  // If the active panel is the Todo Panel, delegate entirely to renderTodoPanel
  if (rightSidebarState.getActivePanel() === 'todos') {
    renderTodoPanel(aside);
    return;
  }

  // 1. Left drag resize handle (overall panel width)
  const resizeHandle = createSidebarResizeHandle();
  aside.appendChild(resizeHandle);

  // 2. Main content container
  const container = document.createElement('div');
  container.className = 'right-sidebar-container';

  // 3. Top Header Bar ("SOURCE CONTROL" + "...")
  const header = createHeader();
  container.appendChild(header);

  // 4. Scrollable Middle Body
  const scrollContent = document.createElement('div');
  scrollContent.className = 'right-sidebar-scroll-content';

  // Determine which visible section is the last expanded section (to flex-fill full height)
  const reposVisible = rightSidebarState.getReposVisible();
  const changesVisible = rightSidebarState.getChangesVisible();
  const graphVisible = rightSidebarState.getGraphVisible();

  const reposExpanded = reposVisible && rightSidebarState.isReposSectionExpanded();
  const changesExpanded = changesVisible && rightSidebarState.isChangesSectionExpanded();
  const graphExpanded = graphVisible && rightSidebarState.isGraphSectionExpanded();

  let flexFillTarget: 'repos' | 'changes' | 'graph' | null = null;
  if (graphExpanded) {
    flexFillTarget = 'graph';
  } else if (changesExpanded) {
    flexFillTarget = 'changes';
  } else if (reposExpanded) {
    flexFillTarget = 'repos';
  }

  // 5. Repositories Section
  if (reposVisible) {
    const isFlexFill = flexFillTarget === 'repos';
    const reposSection = createRepositoriesSection(isFlexFill, reposExpanded && (changesVisible || graphVisible));
    scrollContent.appendChild(reposSection);
  }

  // 6. Changes Section
  if (changesVisible) {
    const isFlexFill = flexFillTarget === 'changes';
    const changesSection = createChangesSection(isFlexFill, changesExpanded && graphVisible);
    scrollContent.appendChild(changesSection);
  }

  // 7. Commit Graph Section
  if (graphVisible) {
    const isFlexFill = flexFillTarget === 'graph';
    const graphSection = createCommitGraphSection(isFlexFill);
    scrollContent.appendChild(graphSection);
  }

  container.appendChild(scrollContent);
  aside.appendChild(container);

  // 8. Render floating context menus if active
  if (activeMenuId) {
    renderContextMenuOverlay(aside);
  }
}

// ============================================================================
// Section 1: Left Drag Resize Handle (Panel Width)
// ============================================================================

/**
 * Creates the left-edge handle allowing users to drag horizontally and resize the panel width.
 */
function createSidebarResizeHandle(): HTMLElement {
  const handle = document.createElement('div');
  handle.className = 'right-sidebar-resize-handle';
  handle.title = 'Drag to resize panel width';

  let startX = 0;
  let startWidth = 0;

  const onMouseMove = (e: MouseEvent) => {
    const delta = startX - e.clientX;
    rightSidebarState.setWidth(startWidth + delta);
  };

  const onMouseUp = () => {
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  };

  handle.addEventListener('mousedown', (e) => {
    startX = e.clientX;
    startWidth = rightSidebarState.getWidth();
    document.body.style.cursor = 'ew-resize';
    document.body.style.userSelect = 'none';
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  });

  return handle;
}

// ============================================================================
// Section 2: Header Component
// ============================================================================

/**
 * Creates the top titlebar of the panel with uppercase title and overflow menu button.
 */
function createHeader(): HTMLElement {
  const header = document.createElement('div');
  header.className = 'sc-header';

  const title = document.createElement('span');
  title.className = 'sc-header-title';
  title.textContent = 'SOURCE CONTROL';

  const actions = document.createElement('div');
  actions.className = 'sc-header-actions';

  const moreBtn = document.createElement('button');
  moreBtn.className = 'sc-icon-btn';
  moreBtn.title = 'More Actions...';
  moreBtn.innerHTML = '<span class="ui-icon icon-sc-ellipsis"></span>';

  moreBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    const rect = moreBtn.getBoundingClientRect();
    openContextMenu('overflow', rect.left - 180, rect.bottom + 4);
  });

  actions.appendChild(moreBtn);
  header.appendChild(title);
  header.appendChild(actions);

  return header;
}

// ============================================================================
// Section 3: Repositories Accordion Component
// ============================================================================

/**
 * Creates the Repositories section.
 * Supports multi-repo selection: clicking any repository row switches the active repo.
 */
function createRepositoriesSection(isFlexFill: boolean, hasDividerBelow: boolean): HTMLElement {
  const section = document.createElement('div');
  const isExpanded = rightSidebarState.isReposSectionExpanded();
  const sectionHeight = rightSidebarState.getReposSectionHeight();

  section.className = `sc-accordion-section ${isExpanded ? (isFlexFill ? 'flex-fill' : '') : 'collapsed'}`;
  if (isExpanded && !isFlexFill) {
    section.style.height = `${sectionHeight}px`;
    section.style.flex = `0 0 ${sectionHeight}px`;
  }

  const repos = rightSidebarState.getRepos();

  // 1. Accordion Header
  const header = document.createElement('div');
  header.className = 'sc-accordion-header';

  const headerLeft = document.createElement('div');
  headerLeft.className = 'sc-accordion-header-left';
  headerLeft.innerHTML = `
    <span class="ui-icon icon-sc-chevron-down sc-chevron ${isExpanded ? '' : 'collapsed'}"></span>
    <span class="sc-accordion-title">REPOSITORIES</span>
    <span class="sc-count-pill">${repos.length}</span>
  `;

  headerLeft.addEventListener('click', () => {
    rightSidebarState.toggleReposSection();
  });

  const headerRight = document.createElement('div');
  headerRight.className = 'sc-accordion-header-right';

  const moreBtn = document.createElement('button');
  moreBtn.className = 'sc-icon-btn';
  moreBtn.title = 'Repository Options';
  moreBtn.innerHTML = '<span class="ui-icon icon-sc-ellipsis"></span>';
  moreBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    const rect = moreBtn.getBoundingClientRect();
    openContextMenu('repo_heading', rect.left - 180, rect.bottom + 4);
  });

  headerRight.appendChild(moreBtn);
  header.appendChild(headerLeft);
  header.appendChild(headerRight);
  section.appendChild(header);

  // 2. Accordion Body (List of repositories)
  if (isExpanded) {
    const list = document.createElement('div');
    list.className = 'sc-accordion-body';

    if (repos.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'sc-empty-row';
      empty.textContent = 'No repositories in workspace';
      list.appendChild(empty);
    } else {
      repos.forEach((repo) => {
        const row = document.createElement('div');
        row.className = `sc-repo-row ${repo.is_active ? 'active' : ''}`;
        row.title = `Click to switch to repository: ${repo.name} (${repo.path || repo.branch})`;

        const rowLeft = document.createElement('div');
        rowLeft.className = 'sc-repo-row-left';
        rowLeft.innerHTML = `
          <span class="ui-icon icon-sc-book sc-repo-icon"></span>
          <span class="sc-repo-name">${repo.name}</span>
        `;

        const rowRight = document.createElement('div');
        rowRight.className = 'sc-repo-row-right';

        const branchSpan = document.createElement('span');
        branchSpan.className = 'sc-repo-branch-text';
        branchSpan.textContent = repo.branch;
        rowRight.appendChild(branchSpan);

        // Hover action: Refresh repo
        const refreshBtn = document.createElement('button');
        refreshBtn.className = 'sc-icon-btn';
        refreshBtn.title = 'Refresh Repository';
        refreshBtn.innerHTML = '<span class="ui-icon icon-sc-refresh-cw"></span>';
        refreshBtn.addEventListener('click', async (e) => {
          e.stopPropagation();
          await refreshRightSidebar(repo.path);
        });
        rowRight.appendChild(refreshBtn);

        // Hover action: Repo options
        const repoMoreBtn = document.createElement('button');
        repoMoreBtn.className = 'sc-icon-btn';
        repoMoreBtn.title = 'Repository Actions';
        repoMoreBtn.innerHTML = '<span class="ui-icon icon-sc-ellipsis"></span>';
        repoMoreBtn.addEventListener('click', (e) => {
          e.stopPropagation();
          const rect = repoMoreBtn.getBoundingClientRect();
          openContextMenu('overflow', rect.left - 180, rect.bottom + 4);
        });
        rowRight.appendChild(repoMoreBtn);

        row.appendChild(rowLeft);
        row.appendChild(rowRight);

        // Clicking a repository switches active repository context immediately
        row.addEventListener('click', async (e) => {
          if ((e.target as HTMLElement).closest('.sc-icon-btn')) return;

          rightSidebarState.setActiveRepoPath(repo.path);
          // Immediate visual feedback
          const updated = rightSidebarState.getRepos().map((r) => ({
            ...r,
            is_active: r.path === repo.path || r.name === repo.name,
          }));
          rightSidebarState.setRepos(updated);
          await refreshRightSidebar(repo.path);
        });

        list.appendChild(row);
      });
    }

    section.appendChild(list);

    // 3. Smooth Resizable Horizontal Divider below Repositories
    if (hasDividerBelow && !isFlexFill) {
      const divider = createSmoothResizeDivider(
        section,
        (newHeight) => rightSidebarState.setReposSectionHeight(newHeight, true),
        60,
        500
      );
      section.appendChild(divider);
    }
  }

  return section;
}

// ============================================================================
// Section 4: Changes & Commit Input Component
// ============================================================================

/**
 * Creates the main Changes section with commit message input, split commit button,
 * and Staged/Unstaged changes accordion subgroups.
 */
function createChangesSection(isFlexFill: boolean, hasDividerBelow: boolean): HTMLElement {
  const section = document.createElement('div');
  const isExpanded = rightSidebarState.isChangesSectionExpanded();
  const sectionHeight = rightSidebarState.getChangesSectionHeight();

  section.className = `sc-accordion-section ${isExpanded ? (isFlexFill ? 'flex-fill' : '') : 'collapsed'}`;
  if (isExpanded && !isFlexFill) {
    section.style.height = `${sectionHeight}px`;
    section.style.flex = `0 0 ${sectionHeight}px`;
  }

  const details = rightSidebarState.getDiffDetails();
  const totalCount = details.staged_files.length + details.unstaged_files.length;

  // 1. Accordion Header
  const header = document.createElement('div');
  header.className = 'sc-accordion-header';

  const headerLeft = document.createElement('div');
  headerLeft.className = 'sc-accordion-header-left';
  headerLeft.innerHTML = `
    <span class="ui-icon icon-sc-chevron-down sc-chevron ${isExpanded ? '' : 'collapsed'}"></span>
    <span class="sc-accordion-title">CHANGES</span>
    <span class="sc-count-pill">${totalCount}</span>
  `;

  headerLeft.addEventListener('click', () => {
    rightSidebarState.toggleChangesSection();
  });

  const headerRight = document.createElement('div');
  headerRight.className = 'sc-accordion-header-right';

  const moreBtn = document.createElement('button');
  moreBtn.className = 'sc-icon-btn';
  moreBtn.title = 'Changes Options';
  moreBtn.innerHTML = '<span class="ui-icon icon-sc-ellipsis"></span>';
  moreBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    const rect = moreBtn.getBoundingClientRect();
    openContextMenu('overflow', rect.left - 180, rect.bottom + 4);
  });

  headerRight.appendChild(moreBtn);
  header.appendChild(headerLeft);
  header.appendChild(headerRight);
  section.appendChild(header);

  // 2. Accordion Body
  if (isExpanded) {
    const body = document.createElement('div');
    body.className = 'sc-accordion-body';

    // A. Commit Message Input Box & Split Commit Button
    const commitBox = createCommitInputBox();
    body.appendChild(commitBox);

    // B. Staged Changes Subgroup (shown only if there are staged files)
    if (details.staged_files.length > 0) {
      const stagedSubgroup = createFilesSubgroup(
        'Staged Changes',
        details.staged_files,
        true,
        rightSidebarState.isStagedSectionExpanded(),
        () => rightSidebarState.toggleStagedSection()
      );
      body.appendChild(stagedSubgroup);
    }

    // C. Unstaged Changes Subgroup
    const unstagedSubgroup = createFilesSubgroup(
      'Changes',
      details.unstaged_files,
      false,
      rightSidebarState.isUnstagedSectionExpanded(),
      () => rightSidebarState.toggleUnstagedSection()
    );
    body.appendChild(unstagedSubgroup);

    section.appendChild(body);

    // 3. Smooth Resizable Horizontal Divider below Changes
    if (hasDividerBelow && !isFlexFill) {
      const divider = createSmoothResizeDivider(
        section,
        (newHeight) => rightSidebarState.setChangesSectionHeight(newHeight, true),
        100,
        700
      );
      section.appendChild(divider);
    }
  }

  return section;
}

/**
 * Creates the Commit message textarea with auto-expansion, AI "Generate" button,
 * and primary split "Commit" button.
 */
function createCommitInputBox(): HTMLElement {
  const container = document.createElement('div');
  container.className = 'sc-commit-section';

  const diffDetails = rightSidebarState.getDiffDetails();
  const branchName = diffDetails.current_branch || 'main';

  // 1. Textarea Container
  const inputContainer = document.createElement('div');
  inputContainer.className = 'sc-commit-input-container';

  const textarea = document.createElement('textarea');
  textarea.className = 'sc-commit-textarea';
  textarea.placeholder = `Message (Ctrl+Enter to commit on "${branchName}")`;
  textarea.value = rightSidebarState.getCommitMessage();
  textarea.rows = 2;

  // Auto-expand textarea height on typing
  textarea.addEventListener('input', () => {
    rightSidebarState.setCommitMessage(textarea.value);
    textarea.style.height = 'auto';
    textarea.style.height = `${Math.min(240, Math.max(52, textarea.scrollHeight))}px`;
  });

  // Ctrl+Enter shortcut to commit
  textarea.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      await executeCommit();
    }
  });

  // 2. AI "Generate" Button overlay in top-right corner
  const genBtn = document.createElement('button');
  genBtn.className = 'sc-btn-generate';
  genBtn.title = 'Generate commit message using AI';
  genBtn.innerHTML = `
    <span class="ui-icon icon-sc-sparkles"></span>
    <span>Generate</span>
  `;

  genBtn.addEventListener('click', async () => {
    if (rightSidebarState.getIsGeneratingMessage()) return;
    rightSidebarState.setIsGeneratingMessage(true);
    genBtn.classList.add('loading');
    try {
      const activePath = resolveCurrentRepoPath();
      const generated = await gitGenerateCommitMessageIpc(activePath);
      rightSidebarState.setCommitMessage(generated);
      textarea.value = generated;
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(240, Math.max(52, textarea.scrollHeight))}px`;
      textarea.focus();
    } catch (err) {
      console.error('[RightSidebar] Failed to generate commit message:', err);
    } finally {
      rightSidebarState.setIsGeneratingMessage(false);
      genBtn.classList.remove('loading');
    }
  });

  inputContainer.appendChild(textarea);
  inputContainer.appendChild(genBtn);

  // 3. Primary Split "Commit" Button
  const splitBtn = document.createElement('div');
  splitBtn.className = 'sc-commit-split-btn';

  const mainCommit = document.createElement('button');
  mainCommit.className = 'sc-commit-main-action';
  mainCommit.innerHTML = `
    <span class="ui-icon icon-sc-check"></span>
    <span>Commit</span>
  `;

  mainCommit.addEventListener('click', async () => {
    await executeCommit();
  });

  const divider = document.createElement('div');
  divider.className = 'sc-split-btn-divider';

  const dropdownCommit = document.createElement('button');
  dropdownCommit.className = 'sc-commit-dropdown-action';
  dropdownCommit.title = 'More Commit Actions';
  dropdownCommit.innerHTML = '<span class="ui-icon icon-sc-chevron-down"></span>';

  dropdownCommit.addEventListener('click', (e) => {
    e.stopPropagation();
    const rect = dropdownCommit.getBoundingClientRect();
    openContextMenu('commit', rect.left - 140, rect.bottom + 4);
  });

  splitBtn.appendChild(mainCommit);
  splitBtn.appendChild(divider);
  splitBtn.appendChild(dropdownCommit);

  container.appendChild(inputContainer);
  container.appendChild(splitBtn);

  return container;
}

/**
 * Helper to create Staged or Unstaged subgroup rows and inline hunk diff preview.
 */
function createFilesSubgroup(
  groupTitle: string,
  files: GitFileDiff[],
  isStaged: boolean,
  isGroupExpanded: boolean,
  onToggleGroup: () => void
): HTMLElement {
  const group = document.createElement('div');
  group.className = 'sc-subgroup-container';

  // 1. Subgroup Header
  const subHeader = document.createElement('div');
  subHeader.className = 'sc-subgroup-header';

  const subLeft = document.createElement('div');
  subLeft.className = 'sc-subgroup-header-left';
  subLeft.innerHTML = `
    <span class="ui-icon icon-sc-chevron-down sc-sub-chevron ${isGroupExpanded ? '' : 'collapsed'}"></span>
    <span class="sc-subgroup-title">${groupTitle}</span>
    <span class="sc-count-pill">${files.length}</span>
  `;

  subLeft.addEventListener('click', onToggleGroup);
  subHeader.appendChild(subLeft);

  const subRight = document.createElement('div');
  subRight.className = 'sc-subgroup-header-right';

  const activePath = resolveCurrentRepoPath();

  if (isStaged) {
    // Unstage all button (-)
    const unstageAllBtn = document.createElement('button');
    unstageAllBtn.className = 'sc-icon-btn';
    unstageAllBtn.title = 'Unstage All';
    unstageAllBtn.innerHTML = '<span class="ui-icon icon-sc-minus"></span>';
    unstageAllBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      await gitUnstageAllFilesIpc(activePath);
      await refreshRightSidebar();
    });
    subRight.appendChild(unstageAllBtn);
  } else {
    // Discard all changes button (undo-2)
    const discardAllBtn = document.createElement('button');
    discardAllBtn.className = 'sc-icon-btn';
    discardAllBtn.title = 'Discard All Changes';
    discardAllBtn.innerHTML = '<span class="ui-icon icon-sc-undo-2"></span>';
    discardAllBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      await gitRevertAllFilesIpc(activePath);
      await refreshRightSidebar();
    });
    subRight.appendChild(discardAllBtn);

    // Stage all changes button (+)
    const stageAllBtn = document.createElement('button');
    stageAllBtn.className = 'sc-icon-btn';
    stageAllBtn.title = 'Stage All';
    stageAllBtn.innerHTML = '<span class="ui-icon icon-sc-plus"></span>';
    stageAllBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      await gitStageAllFilesIpc(activePath);
      await refreshRightSidebar();
    });
    subRight.appendChild(stageAllBtn);
  }

  subHeader.appendChild(subRight);
  group.appendChild(subHeader);

  // 2. Subgroup File List
  if (isGroupExpanded) {
    const filesList = document.createElement('div');
    filesList.className = 'sc-files-list';

    if (files.length === 0) {
      const emptyRow = document.createElement('div');
      emptyRow.className = 'sc-empty-row';
      emptyRow.textContent = 'No changes';
      filesList.appendChild(emptyRow);
    } else {
      files.forEach((file) => {
        const fileItem = document.createElement('div');
        fileItem.className = 'sc-file-item';

        const row = document.createElement('div');
        row.className = 'sc-file-row';

        const rowLeft = document.createElement('div');
        rowLeft.className = 'sc-file-row-left';

        const statusBadgeClass = `status-${file.status.toLowerCase()}`;
        const statusLetter =
          file.status === 'added'
            ? 'A'
            : file.status === 'deleted'
            ? 'D'
            : file.status === 'untracked'
            ? 'U'
            : 'M';

        rowLeft.innerHTML = `
          <span class="ui-icon icon-sc-file-text sc-file-icon"></span>
          <span class="sc-file-name" title="${file.path}">${file.file_name}</span>
          <span class="sc-file-dir">${file.dir_path}</span>
        `;

        rowLeft.addEventListener('click', () => {
          rightSidebarState.toggleFileExpanded(file.path);
        });

        const rowRight = document.createElement('div');
        rowRight.className = 'sc-file-row-right';

        // Hover action buttons
        const actionsContainer = document.createElement('div');
        actionsContainer.className = 'sc-file-actions';

        // Discard button (for unstaged files)
        if (!isStaged) {
          const discardBtn = document.createElement('button');
          discardBtn.className = 'sc-icon-btn sc-action-discard';
          discardBtn.title = 'Discard Changes';
          discardBtn.innerHTML = '<span class="ui-icon icon-sc-undo-2"></span>';
          discardBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            await gitRevertFileIpc(file.path, activePath);
            await refreshRightSidebar();
          });
          actionsContainer.appendChild(discardBtn);
        }

        // Single file Stage (+) or Unstage (-) button
        const toggleStageBtn = document.createElement('button');
        toggleStageBtn.className = `sc-icon-btn ${
          isStaged ? 'sc-action-unstage' : 'sc-action-stage'
        }`;
        toggleStageBtn.title = isStaged ? 'Unstage Changes' : 'Stage Changes';
        toggleStageBtn.innerHTML = `<span class="ui-icon ${
          isStaged ? 'icon-sc-minus' : 'icon-sc-plus'
        }"></span>`;

        toggleStageBtn.addEventListener('click', async (e) => {
          e.stopPropagation();
          if (isStaged) {
            await gitUnstageFileIpc(file.path, activePath);
          } else {
            await gitStageFileIpc(file.path, activePath);
          }
          await refreshRightSidebar();
        });
        actionsContainer.appendChild(toggleStageBtn);

        rowRight.appendChild(actionsContainer);

        // Status Letter badge (M, A, D, U)
        const badge = document.createElement('span');
        badge.className = `sc-status-badge ${statusBadgeClass}`;
        badge.textContent = statusLetter;
        rowRight.appendChild(badge);

        row.appendChild(rowLeft);
        row.appendChild(rowRight);
        fileItem.appendChild(row);

        // Inline Hunk Diff Viewer (when row is clicked)
        if (rightSidebarState.isFileExpanded(file.path)) {
          const diffViewer = createInlineHunkDiffViewer(file);
          fileItem.appendChild(diffViewer);
        }

        filesList.appendChild(fileItem);
      });
    }

    group.appendChild(filesList);
  }

  return group;
}

/**
 * Creates the inline unified hunk diff view with line-by-line colored backgrounds.
 */
function createInlineHunkDiffViewer(file: GitFileDiff): HTMLElement {
  const viewer = document.createElement('div');
  viewer.className = 'sc-hunk-diff-viewer';

  if (!file.hunks || file.hunks.length === 0) {
    const noHunks = document.createElement('div');
    noHunks.className = 'sc-empty-row';
    noHunks.textContent = 'Binary or empty file';
    viewer.appendChild(noHunks);
    return viewer;
  }

  file.hunks.forEach((hunk) => {
    const hunkBox = document.createElement('div');
    hunkBox.className = 'sc-hunk-box';

    // Hunk Header (e.g. @@ -10,4 +10,6 @@)
    const headerRow = document.createElement('div');
    headerRow.className = 'sc-hunk-header-row';
    headerRow.textContent = hunk.header;
    hunkBox.appendChild(headerRow);

    // Diff Lines
    hunk.lines.forEach((line) => {
      const lineRow = document.createElement('div');
      const lineClass =
        line.line_type === '+'
          ? 'sc-line-add'
          : line.line_type === '-'
          ? 'sc-line-del'
          : 'sc-line-ctx';
      lineRow.className = `sc-diff-line-row ${lineClass}`;

      const oldNum = document.createElement('span');
      oldNum.className = 'sc-line-num old';
      oldNum.textContent = line.old_line_num || '';

      const newNum = document.createElement('span');
      newNum.className = 'sc-line-num new';
      newNum.textContent = line.new_line_num || '';

      const typeSign = document.createElement('span');
      typeSign.className = 'sc-line-sign';
      typeSign.textContent = line.line_type;

      const codeContent = document.createElement('span');
      codeContent.className = 'sc-line-code';
      codeContent.textContent = line.content;

      lineRow.appendChild(oldNum);
      lineRow.appendChild(newNum);
      lineRow.appendChild(typeSign);
      lineRow.appendChild(codeContent);

      hunkBox.appendChild(lineRow);
    });

    viewer.appendChild(hunkBox);
  });

  return viewer;
}

// ============================================================================
// Section 5: Commit Graph Component
// ============================================================================

/**
 * Creates the Commit Graph section showing the visual commit history timeline.
 * When expanded as the bottom section, flex-fills full remaining height to avoid dead space.
 */
function createCommitGraphSection(isFlexFill: boolean): HTMLElement {
  const section = document.createElement('div');
  const isExpanded = rightSidebarState.isGraphSectionExpanded();
  const sectionHeight = rightSidebarState.getGraphSectionHeight();

  section.className = `sc-accordion-section ${isExpanded ? (isFlexFill ? 'flex-fill' : '') : 'collapsed'}`;
  if (isExpanded && !isFlexFill) {
    section.style.height = `${sectionHeight}px`;
    section.style.flex = `0 0 ${sectionHeight}px`;
  }

  const commits = rightSidebarState.getGraphCommits();

  // 1. Accordion Header
  const header = document.createElement('div');
  header.className = 'sc-accordion-header';

  const headerLeft = document.createElement('div');
  headerLeft.className = 'sc-accordion-header-left';
  headerLeft.innerHTML = `
    <span class="ui-icon icon-sc-chevron-down sc-chevron ${isExpanded ? '' : 'collapsed'}"></span>
    <span class="sc-accordion-title">COMMIT GRAPH</span>
    <span class="sc-count-pill">${commits.length}</span>
  `;

  headerLeft.addEventListener('click', () => {
    rightSidebarState.toggleGraphSection();
  });

  const headerRight = document.createElement('div');
  headerRight.className = 'sc-accordion-header-right';

  const activePath = resolveCurrentRepoPath();

  // Action: Pull
  const pullBtn = document.createElement('button');
  pullBtn.className = 'sc-icon-btn';
  pullBtn.title = 'Pull';
  pullBtn.innerHTML = '<span class="ui-icon icon-sc-arrow-down"></span>';
  pullBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    await gitPullChangesIpc(undefined, undefined, activePath);
    await refreshRightSidebar();
  });

  // Action: Push
  const pushBtn = document.createElement('button');
  pushBtn.className = 'sc-icon-btn';
  pushBtn.title = 'Push';
  pushBtn.innerHTML = '<span class="ui-icon icon-sc-arrow-up"></span>';
  pushBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    await gitPushChangesIpc(undefined, undefined, activePath);
    await refreshRightSidebar();
  });

  // Action: Filter Branch
  const filterBtn = document.createElement('button');
  filterBtn.className = 'sc-icon-btn';
  filterBtn.title = 'Filter Branch';
  filterBtn.innerHTML = '<span class="ui-icon icon-sc-git-graph"></span>';
  filterBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    await refreshRightSidebar();
  });

  // Action: Center HEAD commit
  const centerHeadBtn = document.createElement('button');
  centerHeadBtn.className = 'sc-icon-btn';
  centerHeadBtn.title = 'Center HEAD Commit';
  centerHeadBtn.innerHTML = '<span class="ui-icon icon-sc-target"></span>';
  centerHeadBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    const headRow = section.querySelector('.sc-graph-row.head');
    headRow?.scrollIntoView({ behavior: 'smooth', block: 'center' });
  });

  // Action: Refresh Graph
  const refreshGraphBtn = document.createElement('button');
  refreshGraphBtn.className = 'sc-icon-btn';
  refreshGraphBtn.title = 'Refresh Commit Graph';
  refreshGraphBtn.innerHTML = '<span class="ui-icon icon-sc-refresh-cw"></span>';
  refreshGraphBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const graph = await getGitCommitGraphIpc(activePath, 50, 0);
    rightSidebarState.setGraphCommits(graph);
  });

  // Action: More Graph Options
  const moreGraphBtn = document.createElement('button');
  moreGraphBtn.className = 'sc-icon-btn';
  moreGraphBtn.title = 'More Graph Actions';
  moreGraphBtn.innerHTML = '<span class="ui-icon icon-sc-ellipsis"></span>';
  moreGraphBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    const rect = moreGraphBtn.getBoundingClientRect();
    openContextMenu('graph_heading', rect.left - 180, rect.bottom + 4);
  });

  headerRight.appendChild(pullBtn);
  headerRight.appendChild(pushBtn);
  headerRight.appendChild(filterBtn);
  headerRight.appendChild(centerHeadBtn);
  headerRight.appendChild(refreshGraphBtn);
  headerRight.appendChild(moreGraphBtn);

  header.appendChild(headerLeft);
  header.appendChild(headerRight);
  section.appendChild(header);

  // 2. Accordion Body (List of commits - full height scroll)
  if (isExpanded) {
    const list = document.createElement('div');
    list.className = 'sc-graph-body';

    if (commits.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'sc-empty-row';
      empty.textContent = 'No commit history found';
      list.appendChild(empty);
    } else {
      commits.forEach((commit) => {
        const row = document.createElement('div');
        row.className = `sc-graph-row ${commit.is_head ? 'head' : ''}`;
        row.title = `Commit ${commit.short_hash}: ${commit.message} (${commit.author}) - Click to copy hash`;

        // 12px visual graph column (2px vertical line + 8px dot)
        const nodeCol = document.createElement('div');
        nodeCol.className = 'sc-graph-node-col';
        nodeCol.innerHTML = `
          <div class="sc-graph-timeline-line"></div>
          <div class="sc-graph-node-circle ${commit.is_head ? 'head' : ''}"></div>
        `;

        // Commit message
        const msg = document.createElement('span');
        msg.className = `sc-graph-msg ${commit.is_head ? 'head' : ''}`;
        msg.textContent = commit.message;

        // Author
        const author = document.createElement('span');
        author.className = 'sc-graph-author';
        author.textContent = commit.author;

        row.appendChild(nodeCol);
        row.appendChild(msg);
        row.appendChild(author);

        // Branch pill if present on HEAD or branch tip
        if (commit.branch_tag) {
          const pill = document.createElement('div');
          pill.className = 'sc-graph-branch-pill';
          pill.innerHTML = `
            <span class="ui-icon icon-sc-git-branch"></span>
            <span class="sc-branch-pill-text">${commit.branch_tag}</span>
            <span class="ui-icon icon-sc-cloud"></span>
          `;
          row.appendChild(pill);
        }

        // Clicking commit row copies commit hash to clipboard
        row.addEventListener('click', async () => {
          try {
            await navigator.clipboard.writeText(commit.hash);
          } catch {
            // fallback
          }
        });

        list.appendChild(row);
      });
    }

    section.appendChild(list);
  }

  return section;
}

// ============================================================================
// Helper: Smooth Section Resizing Divider (Zero DOM recreation during drag)
// ============================================================================

/**
 * Creates a horizontal divider that allows real-time smooth 60fps vertical dragging
 * to resize a section, without wiping or rebuilding the DOM.
 */
function createSmoothResizeDivider(
  targetSection: HTMLElement,
  onSaveHeight: (finalHeight: number) => void,
  minHeight: number,
  maxHeight: number
): HTMLElement {
  const divider = document.createElement('div');
  divider.className = 'sc-section-divider';
  divider.title = 'Drag to resize section';

  let startY = 0;
  let startHeight = 0;

  const onMouseMove = (e: MouseEvent) => {
    const deltaY = e.clientY - startY;
    const newHeight = Math.max(minHeight, Math.min(maxHeight, startHeight + deltaY));
    targetSection.style.height = `${newHeight}px`;
    targetSection.style.flex = `0 0 ${newHeight}px`;
  };

  const onMouseUp = () => {
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
    divider.classList.remove('active');
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    const finalHeight = targetSection.offsetHeight;
    onSaveHeight(finalHeight);
  };

  divider.addEventListener('mousedown', (e) => {
    e.preventDefault();
    startY = e.clientY;
    startHeight = targetSection.offsetHeight;
    divider.classList.add('active');
    document.body.style.cursor = 'ns-resize';
    document.body.style.userSelect = 'none';
    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  });

  return divider;
}

// ============================================================================
// Section 6: Commit Action Execution
// ============================================================================

/**
 * Executes a Git commit with the current message in the textarea for the active repo.
 * @param amend Whether to amend the previous commit.
 */
async function executeCommit(amend = false): Promise<boolean> {
  const msg = rightSidebarState.getCommitMessage().trim();
  if (!msg && !amend) return false;

  if (rightSidebarState.getIsCommitting()) return false;
  rightSidebarState.setIsCommitting(true);

  try {
    const activePath = resolveCurrentRepoPath();
    await gitCommitChangesIpc(msg, amend, activePath);
    rightSidebarState.setCommitMessage('');
    await refreshRightSidebar();
    return true;
  } catch (err) {
    console.error('[RightSidebar] Commit failed:', err);
    return false;
  } finally {
    rightSidebarState.setIsCommitting(false);
  }
}

// ============================================================================
// Section 7: Floating Context Menus & Nested Submenus
// ============================================================================

/**
 * Opens a primary context menu popup overlay at given coordinates.
 */
function openContextMenu(menuId: string, x: number, y: number): void {
  activeMenuId = menuId;
  activeSubmenuItems = null;
  menuCoords = { x, y };
  renderRightSidebar();
}

/**
 * Closes all context menus and nested submenus.
 */
function closeAllMenus(): void {
  activeMenuId = null;
  activeSubmenuItems = null;
}

/**
 * Renders the context menu and optional nested submenu overlays on top of the right sidebar.
 */
function renderContextMenuOverlay(aside: HTMLElement): void {
  const backdrop = document.createElement('div');
  backdrop.className = 'sc-menu-backdrop';
  backdrop.addEventListener('click', () => {
    closeAllMenus();
    renderRightSidebar();
  });

  const menu = document.createElement('div');
  menu.className = 'sc-context-menu-popup';
  menu.style.left = `${Math.max(10, menuCoords.x)}px`;
  menu.style.top = `${menuCoords.y}px`;

  // Select items definition based on activeMenuId
  let items: ContextMenuItem[] = menuDefs.overflowMenu;
  if (activeMenuId === 'commit') {
    items = menuDefs.commitSubmenu;
  } else if (activeMenuId === 'repo_heading') {
    items = menuDefs.overflowMenu;
  } else if (activeMenuId === 'graph_heading') {
    items = menuDefs.graphHeadingMenu;
  }

  items.forEach((item) => {
    if (item.is_separator) {
      const sep = document.createElement('div');
      sep.className = 'sc-menu-separator';
      menu.appendChild(sep);
      return;
    }

    const row = document.createElement('div');
    row.className = `sc-menu-item ${item.is_disabled ? 'disabled' : ''}`;

    const label = document.createElement('span');
    label.className = 'sc-menu-item-label';
    label.textContent = item.label;

    row.appendChild(label);

    if (item.shortcut) {
      const shortcut = document.createElement('span');
      shortcut.className = 'sc-menu-item-shortcut';
      shortcut.textContent = item.shortcut;
      row.appendChild(shortcut);
    }

    if (item.has_submenu) {
      const arrow = document.createElement('span');
      arrow.className = 'ui-icon icon-sc-chevron-right sc-menu-arrow';
      row.appendChild(arrow);

      // On hover, open the corresponding nested submenu
      row.addEventListener('mouseenter', () => {
        const itemRect = row.getBoundingClientRect();
        submenuY = itemRect.top;
        if (item.id === 'view_sub') activeSubmenuItems = menuDefs.viewSubmenu;
        else if (item.id === 'commit_sub') activeSubmenuItems = menuDefs.commitSubmenu;
        else if (item.id === 'changes_sub') activeSubmenuItems = menuDefs.changesSubmenu;
        else if (item.id === 'pull_push_sub') activeSubmenuItems = menuDefs.pullPushSubmenu;
        else if (item.id === 'branch_sub') activeSubmenuItems = menuDefs.branchSubmenu;
        else if (item.id === 'remote_sub') activeSubmenuItems = menuDefs.remoteSubmenu;
        else if (item.id === 'stash_sub') activeSubmenuItems = menuDefs.stashSubmenu;
        else if (item.id === 'tags_sub') activeSubmenuItems = menuDefs.tagsSubmenu;
        else if (item.id === 'worktrees_sub') activeSubmenuItems = menuDefs.worktreesSubmenu;
        renderRightSidebar();
      });
    }

    row.addEventListener('click', async (e) => {
      e.stopPropagation();
      if (item.is_disabled || item.has_submenu) return;
      await handleMenuItemAction(item.id);
      closeAllMenus();
      renderRightSidebar();
    });

    menu.appendChild(row);
  });

  backdrop.appendChild(menu);

  // Render secondary nested submenu if active
  if (activeSubmenuItems) {
    const subMenu = document.createElement('div');
    subMenu.className = 'sc-context-menu-popup sc-nested-submenu';
    subMenu.style.left = `${Math.max(10, menuCoords.x - 210)}px`;
    subMenu.style.top = `${submenuY}px`;

    activeSubmenuItems.forEach((subItem) => {
      if (subItem.is_separator) {
        const sep = document.createElement('div');
        sep.className = 'sc-menu-separator';
        subMenu.appendChild(sep);
        return;
      }

      const subRow = document.createElement('div');
      subRow.className = `sc-menu-item ${subItem.is_disabled ? 'disabled' : ''}`;

      const subLabel = document.createElement('span');
      subLabel.className = 'sc-menu-item-label';
      subLabel.textContent = subItem.label;

      subRow.appendChild(subLabel);

      if (subItem.shortcut) {
        const sc = document.createElement('span');
        sc.className = 'sc-menu-item-shortcut';
        sc.textContent = subItem.shortcut;
        subRow.appendChild(sc);
      }

      subRow.addEventListener('click', async (e) => {
        e.stopPropagation();
        if (subItem.is_disabled) return;
        await handleMenuItemAction(subItem.id);
        closeAllMenus();
        renderRightSidebar();
      });

      subMenu.appendChild(subRow);
    });

    backdrop.appendChild(subMenu);
  }

  aside.appendChild(backdrop);
}

/**
 * Dispatches and executes context menu item actions.
 */
async function handleMenuItemAction(itemId: string): Promise<void> {
  const activePath = resolveCurrentRepoPath();

  switch (itemId) {
    case 'toggle_repos':
      rightSidebarState.toggleReposVisible();
      break;
    case 'toggle_changes':
      rightSidebarState.toggleChangesVisible();
      break;
    case 'toggle_graph':
      rightSidebarState.toggleGraphVisible();
      break;
    case 'cmd_stage_all':
      await gitStageAllFilesIpc(activePath);
      await refreshRightSidebar();
      break;
    case 'cmd_unstage_all':
      await gitUnstageAllFilesIpc(activePath);
      await refreshRightSidebar();
      break;
    case 'cmd_discard_all':
      await gitRevertAllFilesIpc(activePath);
      await refreshRightSidebar();
      break;
    case 'cmd_commit':
      await executeCommit(false);
      break;
    case 'cmd_commit_amend':
      await executeCommit(true);
      break;
    case 'cmd_commit_push': {
      const ok = await executeCommit(false);
      if (ok) {
        await gitPushChangesIpc(undefined, undefined, activePath);
        await refreshRightSidebar();
      }
      break;
    }
    case 'cmd_commit_sync': {
      const ok = await executeCommit(false);
      if (ok) {
        await gitPullChangesIpc(undefined, undefined, activePath);
        await gitPushChangesIpc(undefined, undefined, activePath);
        await refreshRightSidebar();
      }
      break;
    }
    case 'cmd_push':
      await gitPushChangesIpc(undefined, undefined, activePath);
      await refreshRightSidebar();
      break;
    case 'cmd_pull':
      await gitPullChangesIpc(undefined, undefined, activePath);
      await refreshRightSidebar();
      break;
    case 'cmd_fetch':
      await gitFetchChangesIpc(undefined, activePath);
      await refreshRightSidebar();
      break;
    default:
      console.debug('[RightSidebar] Menu item clicked:', itemId);
      break;
  }
}
