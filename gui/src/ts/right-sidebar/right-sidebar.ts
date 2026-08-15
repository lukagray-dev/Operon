// Source Control & Git Diff Right Sidebar Controller & Renderer
//
// 1:1 visual and functional implementation inspired by the Slint GUI right sidebar:
// - VS Code Source Control style panel with resizable left edge.
// - Header with uppercase SOURCE CONTROL title and overflow context menu.
// - Expandable multi-line commit input with AI "Generate" action button and split primary Commit button.
// - Accordion sections: Repositories, Changes (Staged & Unstaged with inline unified hunk diffs), and Commit Graph.
// - Full context menu and nested submenu system.

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
import type { ContextMenuItem, GitFileDiff } from './types.js';

let activeMenuId: string | null = null;
let activeSubmenuItems: ContextMenuItem[] | null = null;
let menuCoords = { x: 0, y: 0 };
let submenuY = 0;

/**
 * Initializes the Right Sidebar component and binds all triggers and keybindings.
 */
export function initRightSidebar(): void {
  // 1. Topbar git-diff toggle button
  const topbarBtn = document.getElementById('btn-topbar-git-commit');
  if (topbarBtn) {
    topbarBtn.addEventListener('click', () => {
      rightSidebarState.toggleOpen();
    });
  }

  // 2. Menu Item "Toggle git-diff panel"
  const menuItemToggle = document.getElementById('menu-item-toggle-git-diff');
  if (menuItemToggle) {
    menuItemToggle.addEventListener('click', () => {
      rightSidebarState.toggleOpen();
    });
  }

  // 3. Global keyboard shortcut Ctrl+G
  window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'g') {
      e.preventDefault();
      rightSidebarState.toggleOpen();
    }
  });

  // 4. Subscribe to state changes to re-render panel
  rightSidebarState.subscribe(() => {
    renderRightSidebar();
  });

  // 5. Subscribe to sidebar project / session switch to refresh Git workspace
  sidebarState.subscribe(() => {
    refreshRightSidebar();
  });

  // 6. Initial fetch of git diff and graph
  refreshRightSidebar();
}

/**
 * Refreshes Git diff details, repositories, and commit graph from backend.
 */
export async function refreshRightSidebar(): Promise<void> {
  const workspacePath = sidebarState.getActiveProjectPath() || undefined;

  try {
    const details = await getGitDiffDetailsIpc(workspacePath);
    rightSidebarState.setDiffDetails(details);

    if (details.has_repo) {
      const repos = await getWorkspaceRepositoriesIpc(workspacePath);
      rightSidebarState.setRepos(repos);

      const graph = await getGitCommitGraphIpc(workspacePath, 40, 0);
      rightSidebarState.setGraphCommits(graph);
    }
  } catch (err) {
    console.warn('[RightSidebar] Failed to refresh Git workspace:', err);
  }
}

/**
 * Master DOM render function for the Right Sidebar.
 */
export function renderRightSidebar(): void {
  const aside = document.getElementById('right-sidebar');
  if (!aside) return;

  const isOpen = rightSidebarState.getIsOpen();
  const width = rightSidebarState.getWidth();

  if (!isOpen) {
    aside.style.display = 'none';
    aside.classList.remove('open');
    closeAllMenus();
    return;
  }

  aside.style.display = 'flex';
  aside.style.width = `${width}px`;
  aside.classList.add('open');
  aside.innerHTML = '';

  // 1. Left drag resize handle
  const resizeHandle = createResizeHandle();
  aside.appendChild(resizeHandle);

  // 2. Main content container
  const container = document.createElement('div');
  container.className = 'right-sidebar-container';

  // 3. Source Control Header
  const header = createHeader();
  container.appendChild(header);

  // 4. Scrollable middle content
  const scrollContent = document.createElement('div');
  scrollContent.className = 'right-sidebar-scroll-content';

  // 5. Commit Input Section
  const commitSection = createCommitInputSection();
  scrollContent.appendChild(commitSection);

  // 6. Repositories Section (if visible)
  if (rightSidebarState.getReposVisible()) {
    const reposSection = createRepositoriesSection();
    scrollContent.appendChild(reposSection);
  }

  // 7. Changes Section (if visible)
  if (rightSidebarState.getChangesVisible()) {
    const changesSection = createChangesSection();
    scrollContent.appendChild(changesSection);
  }

  // 8. Commit Graph Section (if visible)
  if (rightSidebarState.getGraphVisible()) {
    const graphSection = createCommitGraphSection();
    scrollContent.appendChild(graphSection);
  }

  container.appendChild(scrollContent);
  aside.appendChild(container);

  // 9. Render active context menus if open
  if (activeMenuId) {
    renderContextMenuOverlay(aside);
  }
}

/**
 * Creates the left-edge drag resize handle for smooth resizing.
 */
function createResizeHandle(): HTMLElement {
  const handle = document.createElement('div');
  handle.className = 'right-sidebar-resize-handle';
  handle.title = 'Drag to resize panel';

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

/**
 * Creates the uppercase SOURCE CONTROL top header with overflow ellipsis button.
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

/**
 * Creates the Commit message input box, AI Generate button, and primary Commit split button.
 */
function createCommitInputSection(): HTMLElement {
  const section = document.createElement('div');
  section.className = 'sc-commit-section';

  const diffDetails = rightSidebarState.getDiffDetails();
  const branchName = diffDetails.current_branch || 'main';

  // Textarea input container
  const inputContainer = document.createElement('div');
  inputContainer.className = 'sc-commit-input-container';

  const textarea = document.createElement('textarea');
  textarea.className = 'sc-commit-textarea';
  textarea.placeholder = `Message (Ctrl+Enter to commit on "${branchName}")`;
  textarea.value = rightSidebarState.getCommitMessage();
  textarea.rows = 2;

  textarea.addEventListener('input', () => {
    rightSidebarState.setCommitMessage(textarea.value);
  });

  textarea.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      await executeCommit();
    }
  });

  // AI "Generate" button overlay
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
      const workspacePath = sidebarState.getActiveProjectPath() || undefined;
      const generated = await gitGenerateCommitMessageIpc(workspacePath);
      rightSidebarState.setCommitMessage(generated);
      textarea.value = generated;
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

  // Primary Commit Split Button
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

  const sep = document.createElement('div');
  sep.className = 'sc-split-btn-divider';

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
  splitBtn.appendChild(sep);
  splitBtn.appendChild(dropdownCommit);

  section.appendChild(inputContainer);
  section.appendChild(splitBtn);

  return section;
}

/**
 * Creates the Repositories accordion list.
 */
function createRepositoriesSection(): HTMLElement {
  const section = document.createElement('div');
  section.className = 'sc-accordion-section';

  const repos = rightSidebarState.getRepos();
  const isExpanded = rightSidebarState.isReposSectionExpanded();

  // Accordion Header
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

  header.appendChild(headerLeft);
  section.appendChild(header);

  // Accordion Content
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

        const rowLeft = document.createElement('div');
        rowLeft.className = 'sc-repo-row-left';
        rowLeft.innerHTML = `
          <span class="ui-icon icon-sc-folder sc-repo-icon"></span>
          <span class="sc-repo-name">${repo.name}</span>
          <span class="sc-repo-branch-pill">${repo.branch}</span>
        `;

        row.appendChild(rowLeft);
        list.appendChild(row);
      });
    }

    section.appendChild(list);
  }

  return section;
}

/**
 * Creates the Changes accordion section with Staged and Unstaged file groups and inline hunk diffs.
 */
function createChangesSection(): HTMLElement {
  const section = document.createElement('div');
  section.className = 'sc-accordion-section';

  const details = rightSidebarState.getDiffDetails();
  const totalCount = details.staged_files.length + details.unstaged_files.length;
  const isExpanded = rightSidebarState.isChangesSectionExpanded();

  // Accordion Header
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

  // Stage All button (+)
  const stageAllBtn = document.createElement('button');
  stageAllBtn.className = 'sc-icon-btn';
  stageAllBtn.title = 'Stage All Changes';
  stageAllBtn.innerHTML = '<span class="ui-icon icon-sc-plus"></span>';
  stageAllBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const workspacePath = sidebarState.getActiveProjectPath() || undefined;
    await gitStageAllFilesIpc(workspacePath);
    await refreshRightSidebar();
  });

  // Revert All button (undo-2)
  const revertAllBtn = document.createElement('button');
  revertAllBtn.className = 'sc-icon-btn';
  revertAllBtn.title = 'Discard All Changes';
  revertAllBtn.innerHTML = '<span class="ui-icon icon-sc-undo-2"></span>';
  revertAllBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const workspacePath = sidebarState.getActiveProjectPath() || undefined;
    await gitRevertAllFilesIpc(workspacePath);
    await refreshRightSidebar();
  });

  // Refresh button (refresh-cw)
  const refreshBtn = document.createElement('button');
  refreshBtn.className = 'sc-icon-btn';
  refreshBtn.title = 'Refresh Changes';
  refreshBtn.innerHTML = '<span class="ui-icon icon-sc-refresh-cw"></span>';
  refreshBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    await refreshRightSidebar();
  });

  headerRight.appendChild(stageAllBtn);
  headerRight.appendChild(revertAllBtn);
  headerRight.appendChild(refreshBtn);

  header.appendChild(headerLeft);
  header.appendChild(headerRight);
  section.appendChild(header);

  // Accordion Content
  if (isExpanded) {
    const list = document.createElement('div');
    list.className = 'sc-accordion-body';

    // 1. Staged Changes Group
    if (details.staged_files.length > 0) {
      const stagedGroup = createFilesSubgroup(
        'Staged Changes',
        details.staged_files,
        true,
        rightSidebarState.isStagedSectionExpanded(),
        () => rightSidebarState.toggleStagedSection()
      );
      list.appendChild(stagedGroup);
    }

    // 2. Unstaged Changes Group
    const unstagedGroup = createFilesSubgroup(
      'Changes',
      details.unstaged_files,
      false,
      rightSidebarState.isUnstagedSectionExpanded(),
      () => rightSidebarState.toggleUnstagedSection()
    );
    list.appendChild(unstagedGroup);

    section.appendChild(list);
  }

  return section;
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

  // Subgroup Header
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

  if (isStaged) {
    // Unstage all button (-)
    const unstageAllBtn = document.createElement('button');
    unstageAllBtn.className = 'sc-icon-btn';
    unstageAllBtn.title = 'Unstage All';
    unstageAllBtn.innerHTML = '<span class="ui-icon icon-sc-minus"></span>';
    unstageAllBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const workspacePath = sidebarState.getActiveProjectPath() || undefined;
      await gitUnstageAllFilesIpc(workspacePath);
      await refreshRightSidebar();
    });
    subRight.appendChild(unstageAllBtn);
  } else {
    // Stage all button (+)
    const stageAllBtn = document.createElement('button');
    stageAllBtn.className = 'sc-icon-btn';
    stageAllBtn.title = 'Stage All';
    stageAllBtn.innerHTML = '<span class="ui-icon icon-sc-plus"></span>';
    stageAllBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const workspacePath = sidebarState.getActiveProjectPath() || undefined;
      await gitStageAllFilesIpc(workspacePath);
      await refreshRightSidebar();
    });
    subRight.appendChild(stageAllBtn);
  }

  subHeader.appendChild(subRight);
  group.appendChild(subHeader);

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

        // Discard button for unstaged files (undo-2)
        if (!isStaged) {
          const discardBtn = document.createElement('button');
          discardBtn.className = 'sc-icon-btn sc-action-discard';
          discardBtn.title = 'Discard Changes';
          discardBtn.innerHTML = '<span class="ui-icon icon-sc-undo-2"></span>';
          discardBtn.addEventListener('click', async (e) => {
            e.stopPropagation();
            const workspacePath = sidebarState.getActiveProjectPath() || undefined;
            await gitRevertFileIpc(file.path, workspacePath);
            await refreshRightSidebar();
          });
          rowRight.appendChild(discardBtn);
        }

        // Single file Stage (+) or Unstage (-) button
        const toggleStageBtn = document.createElement('button');
        toggleStageBtn.className = 'sc-icon-btn sc-action-stage';
        toggleStageBtn.title = isStaged ? 'Unstage Changes' : 'Stage Changes';
        toggleStageBtn.innerHTML = `<span class="ui-icon ${
          isStaged ? 'icon-sc-minus' : 'icon-sc-plus'
        }"></span>`;

        toggleStageBtn.addEventListener('click', async (e) => {
          e.stopPropagation();
          const workspacePath = sidebarState.getActiveProjectPath() || undefined;
          if (isStaged) {
            await gitUnstageFileIpc(file.path, workspacePath);
          } else {
            await gitStageFileIpc(file.path, workspacePath);
          }
          await refreshRightSidebar();
        });
        rowRight.appendChild(toggleStageBtn);

        // Status Letter badge
        const badge = document.createElement('span');
        badge.className = `sc-status-badge ${statusBadgeClass}`;
        badge.textContent = statusLetter;
        rowRight.appendChild(badge);

        row.appendChild(rowLeft);
        row.appendChild(rowRight);
        fileItem.appendChild(row);

        // Inline Hunk Diff Viewer (when expanded)
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

/**
 * Creates the Commit Graph accordion section with visual commit timeline nodes and branch pills.
 */
function createCommitGraphSection(): HTMLElement {
  const section = document.createElement('div');
  section.className = 'sc-accordion-section';

  const commits = rightSidebarState.getGraphCommits();
  const isExpanded = rightSidebarState.isGraphSectionExpanded();

  // Accordion Header
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

  // Refresh graph button
  const refreshGraphBtn = document.createElement('button');
  refreshGraphBtn.className = 'sc-icon-btn';
  refreshGraphBtn.title = 'Refresh Commit Graph';
  refreshGraphBtn.innerHTML = '<span class="ui-icon icon-sc-refresh-cw"></span>';
  refreshGraphBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    const workspacePath = sidebarState.getActiveProjectPath() || undefined;
    const graph = await getGitCommitGraphIpc(workspacePath, 40, 0);
    rightSidebarState.setGraphCommits(graph);
  });

  headerRight.appendChild(refreshGraphBtn);
  header.appendChild(headerLeft);
  header.appendChild(headerRight);
  section.appendChild(header);

  // Accordion Content
  if (isExpanded) {
    const list = document.createElement('div');
    list.className = 'sc-accordion-body sc-graph-body';

    if (commits.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'sc-empty-row';
      empty.textContent = 'No commit history found';
      list.appendChild(empty);
    } else {
      commits.forEach((commit) => {
        const row = document.createElement('div');
        row.className = `sc-graph-row ${commit.is_head ? 'head' : ''}`;
        row.title = `Commit ${commit.short_hash}: ${commit.message} (${commit.author})`;

        // Left 12px visual graph column (2px vertical line + 8px dot)
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

        // Branch pill if present
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

/**
 * Executes a Git commit with the current message in the textarea.
 */
async function executeCommit(): Promise<void> {
  const msg = rightSidebarState.getCommitMessage().trim();
  if (!msg) return;

  if (rightSidebarState.getIsCommitting()) return;
  rightSidebarState.setIsCommitting(true);

  try {
    const workspacePath = sidebarState.getActiveProjectPath() || undefined;
    await gitCommitChangesIpc(msg, false, workspacePath);
    rightSidebarState.setCommitMessage('');
    await refreshRightSidebar();
  } catch (err) {
    console.error('[RightSidebar] Commit failed:', err);
  } finally {
    rightSidebarState.setIsCommitting(false);
  }
}

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

  const items = activeMenuId === 'commit' ? menuDefs.commitSubmenu : menuDefs.overflowMenu;

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
    subMenu.style.left = `${Math.max(10, menuCoords.x - 190)}px`;
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
 * Handles clicks on context menu items.
 */
async function handleMenuItemAction(itemId: string): Promise<void> {
  const ws = sidebarState.getActiveProjectPath() || undefined;

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
      await gitStageAllFilesIpc(ws);
      await refreshRightSidebar();
      break;
    case 'cmd_unstage_all':
      await gitUnstageAllFilesIpc(ws);
      await refreshRightSidebar();
      break;
    case 'cmd_discard_all':
      await gitRevertAllFilesIpc(ws);
      await refreshRightSidebar();
      break;
    case 'cmd_commit':
      await executeCommit();
      break;
    case 'cmd_push':
      await gitPushChangesIpc(undefined, undefined, ws);
      await refreshRightSidebar();
      break;
    case 'cmd_pull':
      await gitPullChangesIpc(undefined, undefined, ws);
      await refreshRightSidebar();
      break;
    case 'cmd_fetch':
      await gitFetchChangesIpc(undefined, ws);
      await refreshRightSidebar();
      break;
    default:
      console.debug('[RightSidebar] Menu item clicked:', itemId);
      break;
  }
}
