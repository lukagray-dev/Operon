// ============================================================================
// GitHub-Style Unified Diff Viewer Component for Operon Tool Cards
//
// Hey friend! Welcome to the diff viewer DOM component!
// This module takes computed DiffResult hunks and constructs a pixel-perfect,
// GitHub-style unified diff table matching our pure black UI design system.
//
// Key Features:
// 1. Pure Black Background: Base card and context lines remain sleek black (#000000).
// 2. Clear Visual Changes: ONLY added lines have green tint and removed lines have red tint.
// 3. Sticky Dual-Gutter: Line numbers stay pinned on the left during horizontal scrolling.
// 4. Full-Width Backgrounds: Color tints extend across the entire scrollable line length.
// 5. Syntax Highlighting: Integrates seamlessly with `window.hljs` in the detected language.
// ============================================================================

import { detectLanguageFromPath, type DiffHunk, type DiffLine, type DiffResult } from './diff-utils.js';

export interface DiffViewerOptions {
  /** Target file path */
  filePath: string;
  /** Action type label */
  action: 'EDIT' | 'WRITE' | 'APPEND' | 'DELETE';
  /** Computed diff hunks and stats */
  diffResult: DiffResult;
  /** Tool execution status */
  toolStatus: 'running' | 'completed' | 'failed';
  /** Optional human-readable result message */
  resultMessage?: string;
  /** Optional diagnostic list of failed hunks */
  failures?: Array<{ hunk_index?: number; old_string?: string; reason: string }>;
  /** Optional raw text payload to copy when user clicks copy button */
  rawCodeToCopy?: string;
}

/**
 * Escapes raw HTML characters to prevent XSS.
 */
function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Applies highlight.js syntax highlighting to a single line of code safely.
 *
 * @param lineContent - The raw text line to highlight
 * @param lang - Target language name (e.g. 'rust', 'typescript')
 * @returns Highlighted HTML string or escaped plain text
 */
function highlightLineSafe(lineContent: string, lang: string): string {
  if (!lineContent || lineContent.trim().length === 0) {
    return '&nbsp;';
  }

  const hljs = (window as any).hljs;
  if (hljs && lang && lang !== 'text') {
    try {
      const res = hljs.highlight(lineContent, {
        language: lang,
        ignoreIllegals: true,
      });
      return res.value || escapeHtml(lineContent);
    } catch {
      // Fallback on standard HTML escape if hljs throws on unsupported language
    }
  }

  return escapeHtml(lineContent);
}

/**
 * Renders a single diff line row inside a hunk with a pinned sticky gutter.
 */
function renderDiffLineElement(line: DiffLine, lang: string): HTMLElement {
  const row = document.createElement('div');
  const typeClass =
    line.type === 'add'
      ? 'diff-line-add'
      : line.type === 'del'
      ? 'diff-line-del'
      : 'diff-line-ctx';
  row.className = `diff-line-row ${typeClass}`;

  // Gutter container (Sticky on left: 0 so it stays visible during horizontal scroll)
  const gutter = document.createElement('div');
  gutter.className = 'diff-line-gutter';

  // 1. Old line number gutter (left column)
  const oldGutter = document.createElement('span');
  oldGutter.className = 'diff-gutter-num old-num';
  oldGutter.textContent = line.oldLineNum !== undefined ? String(line.oldLineNum) : '';
  gutter.appendChild(oldGutter);

  // 2. New line number gutter (right column)
  const newGutter = document.createElement('span');
  newGutter.className = 'diff-gutter-num new-num';
  newGutter.textContent = line.newLineNum !== undefined ? String(line.newLineNum) : '';
  gutter.appendChild(newGutter);

  // 3. Diff sign marker (+, -, or empty space)
  const marker = document.createElement('span');
  marker.className = 'diff-marker';
  marker.textContent = line.type === 'add' ? '+' : line.type === 'del' ? '-' : ' ';
  gutter.appendChild(marker);

  row.appendChild(gutter);

  // 4. Code content with syntax highlighting
  const codeEl = document.createElement('span');
  codeEl.className = 'diff-code-text';
  codeEl.innerHTML = highlightLineSafe(line.content, lang);
  row.appendChild(codeEl);

  return row;
}

/**
 * Renders a single diff hunk block of lines without any hunk heading banner.
 */
function renderHunkElement(hunk: DiffHunk, lang: string): HTMLElement {
  const hunkBox = document.createElement('div');
  hunkBox.className = 'diff-hunk-box';

  if (!hunk.lines || hunk.lines.length === 0) {
    const emptyNotice = document.createElement('div');
    emptyNotice.className = 'diff-empty-notice';
    emptyNotice.textContent = '(Empty file or no content)';
    hunkBox.appendChild(emptyNotice);
  } else {
    // Render Diff Lines directly (no blue @@ +/- banner)
    hunk.lines.forEach((line) => {
      hunkBox.appendChild(renderDiffLineElement(line, lang));
    });
  }

  return hunkBox;
}

/**
 * Constructs the complete GitHub-style Unified Diff Card DOM element.
 *
 * @param options - Parameters describing file path, hunks, stats, and tool status
 * @returns Fully styled diff viewer HTMLElement
 */
export function createDiffViewerElement(options: DiffViewerOptions): HTMLElement {
  const { filePath, diffResult, toolStatus, resultMessage, failures, rawCodeToCopy } = options;
  const lang = detectLanguageFromPath(filePath);

  const container = document.createElement('div');
  container.className = `diff-viewer-card status-${toolStatus}`;

  // Prevent scroll propagation from hijacking chat messages viewport
  container.addEventListener(
    'wheel',
    (e) => {
      e.stopPropagation();
    },
    { passive: true }
  );

  // --------------------------------------------------------------------------
  // 1. Top Diff Card Header Bar
  // --------------------------------------------------------------------------
  const header = document.createElement('div');
  header.className = 'diff-viewer-header';

  // Left group: File path + Plain stats text (+N / -M without container)
  const leftGroup = document.createElement('div');
  leftGroup.className = 'diff-header-left';

  // Split path for prominent basename and muted directory prefix
  const pathParts = filePath.split(/[/\\]/);
  const baseName = pathParts.pop() || filePath;
  const dirPrefix = pathParts.join('/');

  const pathLabel = document.createElement('div');
  pathLabel.className = 'diff-path-label';
  pathLabel.title = filePath;

  if (dirPrefix) {
    const dirSpan = document.createElement('span');
    dirSpan.className = 'diff-dir-prefix';
    dirSpan.textContent = `${dirPrefix}/`;
    pathLabel.appendChild(dirSpan);
  }

  const baseSpan = document.createElement('span');
  baseSpan.className = 'diff-base-name';
  baseSpan.textContent = baseName;
  pathLabel.appendChild(baseSpan);
  leftGroup.appendChild(pathLabel);

  // Stats text (+N / -M) without any container box
  const { insertions, deletions } = diffResult.stats;
  if (insertions > 0 || deletions > 0) {
    const statsGroup = document.createElement('div');
    statsGroup.className = 'diff-stats-text-group';

    if (insertions > 0) {
      const addSpan = document.createElement('span');
      addSpan.className = 'stat-add-text';
      addSpan.textContent = `+${insertions}`;
      statsGroup.appendChild(addSpan);
    }

    if (deletions > 0) {
      const delSpan = document.createElement('span');
      delSpan.className = 'stat-del-text';
      delSpan.textContent = `-${deletions}`;
      statsGroup.appendChild(delSpan);
    }

    leftGroup.appendChild(statsGroup);
  }

  header.appendChild(leftGroup);

  // Right group: Applied/Failed/Running SVG status icon + Copy button
  const rightGroup = document.createElement('div');
  rightGroup.className = 'diff-header-right';

  // Status SVG Icon
  const statusIcon = document.createElement('span');
  statusIcon.className = `diff-status-icon status-${toolStatus}`;
  if (toolStatus === 'completed') {
    statusIcon.title = 'Applied';
    statusIcon.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#3fb950" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"></polyline>
      </svg>`;
  } else if (toolStatus === 'failed') {
    statusIcon.title = 'Failed';
    statusIcon.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#f85149" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"></line>
        <line x1="6" y1="6" x2="18" y2="18"></line>
      </svg>`;
  } else {
    statusIcon.title = 'Applying...';
    statusIcon.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#e3b341" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="diff-spinner-svg">
        <path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
      </svg>`;
  }
  rightGroup.appendChild(statusIcon);

  // Copy code button (uses rawCodeToCopy or reconstructs changed text)
  const copyBtn = document.createElement('button');
  copyBtn.className = 'diff-copy-btn';
  copyBtn.type = 'button';
  copyBtn.title = 'Copy diff text';
  copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';

  copyBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      const textToCopy =
        rawCodeToCopy ||
        diffResult.hunks
          .map((h) => `${h.header}\n${h.lines.map((l) => `${l.type === 'add' ? '+' : l.type === 'del' ? '-' : ' '}${l.content}`).join('\n')}`)
          .join('\n\n');
      await navigator.clipboard.writeText(textToCopy);
      copyBtn.classList.add('copied');
      copyBtn.innerHTML = '<span class="ui-icon icon-diff-check"></span>';
      setTimeout(() => {
        copyBtn.classList.remove('copied');
        copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
      }, 2000);
    } catch (err) {
      console.debug('[DiffViewer] Failed to copy diff text:', err);
    }
  });
  rightGroup.appendChild(copyBtn);

  header.appendChild(rightGroup);
  container.appendChild(header);

  // --------------------------------------------------------------------------
  // 2. Diff Body with Hunks & Gutter Rows
  // --------------------------------------------------------------------------
  const body = document.createElement('div');
  body.className = 'diff-viewer-body';

  const hasAnyLines =
    diffResult.hunks &&
    diffResult.hunks.some((h) => Array.isArray(h.lines) && h.lines.length > 0);

  if (!hasAnyLines) {
    const emptyNotice = document.createElement('div');
    emptyNotice.className = 'diff-empty-notice';
    emptyNotice.textContent =
      toolStatus === 'running'
        ? 'Generating diff changes...'
        : 'No text changes to display';
    body.appendChild(emptyNotice);
  } else {
    diffResult.hunks.forEach((hunk) => {
      body.appendChild(renderHunkElement(hunk, lang));
    });
  }

  container.appendChild(body);

  // --------------------------------------------------------------------------
  // 3. Footer Section (Execution Message & Diagnostics)
  // --------------------------------------------------------------------------
  if (failures && failures.length > 0) {
    const failureBox = document.createElement('div');
    failureBox.className = 'diff-failure-box';

    failures.forEach((f) => {
      const item = document.createElement('div');
      item.className = 'diff-failure-item';
      item.innerHTML = `
        <span class="diff-failure-icon">✕</span>
        <span class="diff-failure-reason">${escapeHtml(f.reason || 'Hunk failed to match')}</span>
      `;
      failureBox.appendChild(item);
    });

    container.appendChild(failureBox);
  } else if (resultMessage && resultMessage.trim().length > 0) {
    const footer = document.createElement('div');
    footer.className = `diff-viewer-footer ${toolStatus === 'failed' ? 'failed' : 'success'}`;
    footer.textContent = resultMessage;
    container.appendChild(footer);
  }

  return container;
}
