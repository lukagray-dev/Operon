// ============================================================================
// Specialized Tool Card Renderers for Operon Chat Timeline
//
// Hey friend! Welcome to the specialized tool card renderers module!
// This file transforms raw tool execution payloads into polished, minimal UI cards:
//
// 1. File Modification Cards (write, edit, append, delete):
//    - Delegates to `diff-viewer.ts` for GitHub-style unified diffs.
//    - Header shows ONLY file path, unboxed +N / -M stats, status SVG, and copy button.
// 2. File Reading Cards (read, view_file):
//    - Single scroll container with sticky line numbers and syntax highlighting.
//    - Header shows ONLY file path, status SVG, and copy button.
// 3. Command Execution Cards (bash, exec, run_command):
//    - Dark terminal prompt `$ command`, directory chip, status SVG, and stdout/stderr box.
// 4. Codebase Search & Grep Cards:
//    - Search query chip, directory scope, status SVG, and match output.
// 5. Web Fetch Cards:
//    - Clean URL badge, status SVG, and response box.
// 6. Generic Fallback Cards:
//    - Formatted argument chips, status SVG, and JSON viewer.
// ============================================================================

import {
  computeAppendDiff,
  computeDiffBetweenTexts,
  computeWriteDiff,
  detectLanguageFromPath,
  type DiffHunk,
  type DiffResult,
} from './diff-utils.js';
import { createDiffViewerElement } from './diff-viewer.js';
import type { WorkGroupToolItem } from './types.js';

/**
 * Escapes unsafe characters for direct HTML insertion.
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
 * Safely parses raw JSON or objects (handles double/triple-serialized JSON strings or raw objects).
 */
function parseJsonSafe(raw: any): Record<string, any> {
  if (!raw) return {};
  if (typeof raw === 'object' && raw !== null) return raw;
  if (typeof raw !== 'string') return {};
  try {
    let parsed = JSON.parse(raw);
    while (typeof parsed === 'string') {
      try {
        parsed = JSON.parse(parsed);
      } catch {
        break;
      }
    }
    if (typeof parsed === 'object' && parsed !== null) {
      return parsed;
    }
    return {};
  } catch {
    return {};
  }
}

/**
 * Helper to safely highlight a line using highlight.js or fallback to escaped HTML.
 */
function highlightCodeLineSafe(lineContent: string, lang: string): string {
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
      // Fallback
    }
  }

  return escapeHtml(lineContent);
}

/**
 * Generates an SVG status indicator (checkmark for completed, cross for failed, spinner for running).
 */
function createStatusSvgIcon(status: string): HTMLElement {
  const statusIcon = document.createElement('span');
  statusIcon.className = `diff-status-icon status-${status}`;
  if (status === 'completed') {
    statusIcon.title = 'Completed';
    statusIcon.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#3fb950" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"></polyline>
      </svg>`;
  } else if (status === 'failed') {
    statusIcon.title = 'Failed';
    statusIcon.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#f85149" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"></line>
        <line x1="6" y1="6" x2="18" y2="18"></line>
      </svg>`;
  } else {
    statusIcon.title = 'Running...';
    statusIcon.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#e3b341" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="diff-spinner-svg">
        <path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
      </svg>`;
  }
  return statusIcon;
}

// ----------------------------------------------------------------------------
// Section 1: File Modification Tool Card Renderer (Diffs)
// ----------------------------------------------------------------------------

/**
 * Parses file modification tool calls (edit, write, append, delete) and renders
 * a GitHub-style unified diff card.
 */
export function renderFileModificationCard(item: WorkGroupToolItem): HTMLElement {
  const name = (item.tool_name || '').toLowerCase();
  const args = parseJsonSafe(item.tool_args);
  const result = parseJsonSafe(item.tool_result);

  const rawPath =
    (args.path ||
      args.file_path ||
      args.filePath ||
      args.filepath ||
      args.target_file ||
      args.targetFile ||
      args.TargetFile ||
      args.AbsolutePath ||
      args.file ||
      args.filename ||
      args.fileName ||
      args.destination ||
      args.dest ||
      args.uri ||
      args.relative_path ||
      args.relativePath ||
      'file') as string;

  // A. Write / Create Tool Identification
  const isWrite =
    name === 'write' ||
    name === 'write_to_file' ||
    name === 'create_file' ||
    name === 'save_file' ||
    name === 'write_file' ||
    name === 'new_file';

  const newContent =
    args.content ??
    args.CodeContent ??
    args.code ??
    args.text ??
    args.body ??
    args.__body__ ??
    args.file_content ??
    args.fileContent ??
    args.contents ??
    args.data ??
    args.source ??
    args.content_str ??
    args.new_content ??
    args.newContent;

  if (
    isWrite ||
    (newContent !== undefined &&
      !args.edits &&
      !args.replacements &&
      args.old_string === undefined &&
      args.old_str === undefined &&
      args.TargetContent === undefined &&
      args.search === undefined &&
      name !== 'append' &&
      name !== 'append_to_file' &&
      name !== 'append_file' &&
      name !== 'delete' &&
      name !== 'delete_file')
  ) {
    const textToWrite =
      typeof newContent === 'string'
        ? newContent
        : newContent !== undefined
        ? String(newContent)
        : '';
    const diffResult = computeWriteDiff(textToWrite);
    return createDiffViewerElement({
      filePath: rawPath,
      action: 'WRITE',
      diffResult,
      toolStatus: item.tool_status,
      resultMessage:
        result.message ||
        (item.tool_status === 'completed' ? 'File written successfully' : ''),
      rawCodeToCopy: textToWrite,
    });
  }

  // B. Append Tool
  const isAppend =
    name === 'append' ||
    name === 'append_to_file' ||
    name === 'append_file';

  if (isAppend) {
    const appendContent =
      args.content ??
      args.text ??
      args.body ??
      args.data ??
      args.CodeContent ??
      args.code ??
      args.file_content ??
      args.fileContent ??
      args.contents ??
      '';
    const textToAppend =
      typeof appendContent === 'string' ? appendContent : String(appendContent);
    const diffResult = computeAppendDiff(textToAppend);
    return createDiffViewerElement({
      filePath: rawPath,
      action: 'APPEND',
      diffResult,
      toolStatus: item.tool_status,
      resultMessage:
        result.message ||
        (item.tool_status === 'completed' ? 'Appended to file successfully' : ''),
      rawCodeToCopy: textToAppend,
    });
  }

  // C. Delete Tool
  const isDelete =
    name === 'delete' ||
    name === 'delete_file' ||
    name === 'remove_file';

  if (isDelete) {
    const emptyDiff: DiffResult = { hunks: [], stats: { insertions: 0, deletions: 0 } };
    return createDiffViewerElement({
      filePath: rawPath,
      action: 'DELETE',
      diffResult: emptyDiff,
      toolStatus: item.tool_status,
      resultMessage:
        result.message || (item.tool_status === 'completed' ? 'File deleted' : ''),
    });
  }

  // D. Edit Tool (Search & Replace / Multi-hunks)
  let hunksList: Array<{ old_string: string; new_string: string; startLine?: number }> = [];

  if (Array.isArray(args.edits) && args.edits.length > 0) {
    hunksList = args.edits.map((h: any) => ({
      old_string:
        h.old_string ??
        h.old_str ??
        h.oldStr ??
        h.TargetContent ??
        h.targetContent ??
        h.search ??
        h.find ??
        h.oldText ??
        h.old ??
        '',
      new_string:
        h.new_string ??
        h.new_str ??
        h.newStr ??
        h.ReplacementContent ??
        h.replacementContent ??
        h.replacement ??
        h.replace ??
        h.newText ??
        h.new ??
        '',
      startLine: h.start_line ?? h.startLine ?? h.StartLine,
    }));
  } else if (Array.isArray(args.replacements) && args.replacements.length > 0) {
    hunksList = args.replacements.map((h: any) => ({
      old_string:
        h.old_string ??
        h.old_str ??
        h.oldStr ??
        h.TargetContent ??
        h.targetContent ??
        h.search ??
        h.find ??
        h.oldText ??
        h.old ??
        '',
      new_string:
        h.new_string ??
        h.new_str ??
        h.newStr ??
        h.ReplacementContent ??
        h.replacementContent ??
        h.replacement ??
        h.replace ??
        h.newText ??
        h.new ??
        '',
      startLine: h.start_line ?? h.startLine ?? h.StartLine,
    }));
  } else if (
    args.old_string !== undefined ||
    args.old_str !== undefined ||
    args.oldStr !== undefined ||
    args.TargetContent !== undefined ||
    args.targetContent !== undefined ||
    args.search !== undefined ||
    args.find !== undefined ||
    args.old !== undefined ||
    args.oldText !== undefined
  ) {
    const oldStr =
      args.old_string ??
      args.old_str ??
      args.oldStr ??
      args.TargetContent ??
      args.targetContent ??
      args.search ??
      args.find ??
      args.old ??
      args.oldText ??
      '';
    const newStr =
      args.new_string ??
      args.new_str ??
      args.newStr ??
      args.ReplacementContent ??
      args.replacementContent ??
      args.replacement ??
      args.replace ??
      args.new ??
      args.newText ??
      '';
    hunksList.push({
      old_string: typeof oldStr === 'string' ? oldStr : String(oldStr),
      new_string: typeof newStr === 'string' ? newStr : String(newStr),
      startLine: args.start_line ?? args.startLine ?? args.StartLine ?? 1,
    });
  }

  // Fallback: If no edit hunks matched but there is content or text, treat as write diff
  if (hunksList.length === 0 && newContent !== undefined) {
    const textToWrite =
      typeof newContent === 'string' ? newContent : String(newContent);
    const diffResult = computeWriteDiff(textToWrite);
    return createDiffViewerElement({
      filePath: rawPath,
      action: 'WRITE',
      diffResult,
      toolStatus: item.tool_status,
      resultMessage:
        result.message ||
        (item.tool_status === 'completed' ? 'File written successfully' : ''),
      rawCodeToCopy: textToWrite,
    });
  }

  // Combine computed diff hunks from all edit operations
  const combinedHunks: DiffHunk[] = [];
  let totalInsertions = 0;
  let totalDeletions = 0;

  hunksList.forEach((h, idx) => {
    const startLine = h.startLine || (idx + 1) * 10;
    const diff = computeDiffBetweenTexts(h.old_string, h.new_string, startLine);
    diff.hunks.forEach((hunk) => {
      combinedHunks.push(hunk);
    });
    totalInsertions += diff.stats.insertions;
    totalDeletions += diff.stats.deletions;
  });

  const diffResult: DiffResult = {
    hunks: combinedHunks,
    stats: {
      insertions: totalInsertions,
      deletions: totalDeletions,
    },
  };

  const failures = Array.isArray(result.failures) ? result.failures : undefined;
  const resultMessage =
    result.message ||
    (item.tool_status === 'completed'
      ? 'All edit hunks applied successfully'
      : item.tool_status === 'running'
      ? 'Applying edits...'
      : '');

  return createDiffViewerElement({
    filePath: rawPath,
    action: 'EDIT',
    diffResult,
    toolStatus: item.tool_status,
    resultMessage,
    failures,
  });
}

// ----------------------------------------------------------------------------
// Section 2: File Reading Tool Card Renderer
// ----------------------------------------------------------------------------

/**
 * Renders file read operations as a clean code viewer card with aligned line numbers and single scroll.
 */
export function renderFileReadCard(item: WorkGroupToolItem): HTMLElement {
  const args = parseJsonSafe(item.tool_args);
  const rawPath = (args.path ||
    args.file_path ||
    args.filePath ||
    args.filepath ||
    args.target_file ||
    args.TargetFile ||
    args.AbsolutePath ||
    'file') as string;
  const lang = detectLanguageFromPath(rawPath);

  const startLine = Number(
    args.offset_line || args.StartLine || args.startLine || args.offset || 1
  );
  const rawContent =
    typeof item.tool_result === 'string'
      ? item.tool_result
      : JSON.stringify(item.tool_result, null, 2);

  const card = document.createElement('div');
  card.className = 'tool-custom-card tool-read-card';

  // Prevent scroll hijacking
  card.addEventListener('wheel', (e) => e.stopPropagation(), { passive: true });

  const pathParts = rawPath.split(/[/\\]/);
  const baseName = pathParts.pop() || rawPath;
  const dirPrefix = pathParts.join('/');

  const header = document.createElement('div');
  header.className = 'tool-custom-header';

  const leftGroup = document.createElement('div');
  leftGroup.className = 'tool-header-left';
  leftGroup.innerHTML = `
    <div class="diff-path-label">
      ${dirPrefix ? `<span class="diff-dir-prefix">${escapeHtml(dirPrefix)}/</span>` : ''}
      <span class="diff-base-name">${escapeHtml(baseName)}</span>
    </div>
  `;
  header.appendChild(leftGroup);

  const rightGroup = document.createElement('div');
  rightGroup.className = 'tool-header-right';

  rightGroup.appendChild(createStatusSvgIcon(item.tool_status));

  const copyBtn = document.createElement('button');
  copyBtn.className = 'diff-copy-btn';
  copyBtn.type = 'button';
  copyBtn.title = 'Copy file content';
  copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
  copyBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(rawContent);
      copyBtn.classList.add('copied');
      copyBtn.innerHTML = '<span class="ui-icon icon-diff-check"></span>';
      setTimeout(() => {
        copyBtn.classList.remove('copied');
        copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
      }, 2000);
    } catch {}
  });
  rightGroup.appendChild(copyBtn);
  header.appendChild(rightGroup);
  card.appendChild(header);

  // Single unified scroll viewport container
  const viewport = document.createElement('div');
  viewport.className = 'tool-read-viewport';

  const linesContainer = document.createElement('div');
  linesContainer.className = 'tool-read-lines-container';

  if (!rawContent || rawContent.trim().length === 0) {
    const emptyEl = document.createElement('div');
    emptyEl.className = 'diff-empty-notice';
    emptyEl.textContent =
      item.tool_status === 'running'
        ? 'Reading file content...'
        : '(Empty file or no content returned)';
    linesContainer.appendChild(emptyEl);
  } else {
    const lines = rawContent.split(/\r?\n/);
    lines.forEach((lineText, idx) => {
      const lineRow = document.createElement('div');
      lineRow.className = 'tool-read-line-row';

      // Sticky line number gutter on the left
      const numSpan = document.createElement('span');
      numSpan.className = 'tool-read-gutter-num';
      numSpan.textContent = String(startLine + idx);
      lineRow.appendChild(numSpan);

      // Code text with syntax highlighting
      const codeSpan = document.createElement('span');
      codeSpan.className = 'tool-read-code-text';
      codeSpan.innerHTML = highlightCodeLineSafe(lineText, lang);
      lineRow.appendChild(codeSpan);

      linesContainer.appendChild(lineRow);
    });
  }

  viewport.appendChild(linesContainer);
  card.appendChild(viewport);

  return card;
}

// ----------------------------------------------------------------------------
// Section 3: Terminal / Command Execution Tool Card Renderer
// ----------------------------------------------------------------------------

/**
 * Renders shell command execution with dark terminal prompt and stdout/stderr box.
 */
export function renderCommandCard(item: WorkGroupToolItem): HTMLElement {
  const args = parseJsonSafe(item.tool_args);
  const command =
    args.CommandLine || args.command || args.cmd || args.CommandLineString || '';
  const cwd = args.Cwd || args.cwd || args.dir || '';

  const card = document.createElement('div');
  card.className = `tool-custom-card tool-cmd-card status-${item.tool_status}`;

  card.addEventListener('wheel', (e) => e.stopPropagation(), { passive: true });

  const header = document.createElement('div');
  header.className = 'tool-custom-header';

  const leftGroup = document.createElement('div');
  leftGroup.className = 'tool-header-left';
  leftGroup.innerHTML = `
    <span class="tool-cmd-prompt">$</span>
    <span class="tool-cmd-string" title="${escapeHtml(command)}">${escapeHtml(command || 'Command')}</span>
    ${cwd ? `<span class="tool-cwd-chip" title="Directory: ${escapeHtml(cwd)}">${escapeHtml(cwd.split(/[/\\]/).pop() || cwd)}</span>` : ''}
  `;
  header.appendChild(leftGroup);

  const rightGroup = document.createElement('div');
  rightGroup.className = 'tool-header-right';
  rightGroup.appendChild(createStatusSvgIcon(item.tool_status));

  const copyBtn = document.createElement('button');
  copyBtn.className = 'diff-copy-btn';
  copyBtn.type = 'button';
  copyBtn.title = 'Copy command and output';
  copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
  copyBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      const payload = `$ ${command}\n${item.tool_result || ''}`;
      await navigator.clipboard.writeText(payload);
      copyBtn.classList.add('copied');
      copyBtn.innerHTML = '<span class="ui-icon icon-diff-check"></span>';
      setTimeout(() => {
        copyBtn.classList.remove('copied');
        copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
      }, 2000);
    } catch {}
  });
  rightGroup.appendChild(copyBtn);
  header.appendChild(rightGroup);
  card.appendChild(header);

  const termBody = document.createElement('div');
  termBody.className = 'tool-terminal-body';
  termBody.innerHTML = `<pre class="tool-terminal-pre"><code>${escapeHtml(item.tool_result || (item.tool_status === 'running' ? 'Executing command...' : '(no output)'))}</code></pre>`;
  card.appendChild(termBody);

  return card;
}

// ----------------------------------------------------------------------------
// Section 4: Codebase Search & Grep Tool Card Renderer
// ----------------------------------------------------------------------------

/**
 * Renders codebase search and grep results into structured match items.
 */
export function renderSearchCard(item: WorkGroupToolItem): HTMLElement {
  const args = parseJsonSafe(item.tool_args);
  const query = args.Query || args.query || args.pattern || args.Pattern || '';
  const searchPath =
    args.SearchPath || args.SearchDirectory || args.path || args.dir || '';

  const card = document.createElement('div');
  card.className = `tool-custom-card tool-search-card status-${item.tool_status}`;

  card.addEventListener('wheel', (e) => e.stopPropagation(), { passive: true });

  const header = document.createElement('div');
  header.className = 'tool-custom-header';

  const leftGroup = document.createElement('div');
  leftGroup.className = 'tool-header-left';
  leftGroup.innerHTML = `
    <span class="tool-search-query">"${escapeHtml(query)}"</span>
    ${searchPath ? `<span class="tool-search-path">${escapeHtml(searchPath)}</span>` : ''}
  `;
  header.appendChild(leftGroup);

  const rightGroup = document.createElement('div');
  rightGroup.className = 'tool-header-right';
  rightGroup.appendChild(createStatusSvgIcon(item.tool_status));

  const copyBtn = document.createElement('button');
  copyBtn.className = 'diff-copy-btn';
  copyBtn.type = 'button';
  copyBtn.title = 'Copy results';
  copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
  copyBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(item.tool_result || '');
      copyBtn.classList.add('copied');
      copyBtn.innerHTML = '<span class="ui-icon icon-diff-check"></span>';
      setTimeout(() => {
        copyBtn.classList.remove('copied');
        copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
      }, 2000);
    } catch {}
  });
  rightGroup.appendChild(copyBtn);
  header.appendChild(rightGroup);
  card.appendChild(header);

  const searchBody = document.createElement('div');
  searchBody.className = 'tool-search-body';
  searchBody.innerHTML = `<pre class="tool-search-pre"><code>${escapeHtml(item.tool_result || (item.tool_status === 'running' ? 'Searching codebase...' : '(no matches)'))}</code></pre>`;
  card.appendChild(searchBody);

  return card;
}

// ----------------------------------------------------------------------------
// Section 5: Web Fetch & Search Tool Card Renderer
// ----------------------------------------------------------------------------

/**
 * Renders web search or page fetch operations with URL badge.
 */
export function renderWebCard(item: WorkGroupToolItem): HTMLElement {
  const args = parseJsonSafe(item.tool_args);
  const targetUrl = args.Url || args.url || args.query || args.Query || '';

  const card = document.createElement('div');
  card.className = `tool-custom-card tool-web-card status-${item.tool_status}`;

  card.addEventListener('wheel', (e) => e.stopPropagation(), { passive: true });

  const header = document.createElement('div');
  header.className = 'tool-custom-header';

  const leftGroup = document.createElement('div');
  leftGroup.className = 'tool-header-left';
  leftGroup.innerHTML = `
    <span class="tool-web-url" title="${escapeHtml(targetUrl)}">${escapeHtml(targetUrl)}</span>
  `;
  header.appendChild(leftGroup);

  const rightGroup = document.createElement('div');
  rightGroup.className = 'tool-header-right';
  rightGroup.appendChild(createStatusSvgIcon(item.tool_status));

  const copyBtn = document.createElement('button');
  copyBtn.className = 'diff-copy-btn';
  copyBtn.type = 'button';
  copyBtn.title = 'Copy content';
  copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
  copyBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(item.tool_result || '');
      copyBtn.classList.add('copied');
      copyBtn.innerHTML = '<span class="ui-icon icon-diff-check"></span>';
      setTimeout(() => {
        copyBtn.classList.remove('copied');
        copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
      }, 2000);
    } catch {}
  });
  rightGroup.appendChild(copyBtn);
  header.appendChild(rightGroup);
  card.appendChild(header);

  const webBody = document.createElement('div');
  webBody.className = 'tool-search-body';
  webBody.innerHTML = `<pre class="tool-search-pre"><code>${escapeHtml(item.tool_result || (item.tool_status === 'running' ? 'Fetching web content...' : '(no content)'))}</code></pre>`;
  card.appendChild(webBody);

  return card;
}

// ----------------------------------------------------------------------------
// Section 6: Generic & Fallback Tool Card Renderer
// ----------------------------------------------------------------------------

/**
 * Fallback card renderer for custom or miscellaneous tools.
 * Formats JSON arguments into clean chips and presents syntax-highlighted output.
 */
export function renderGenericToolCard(item: WorkGroupToolItem): HTMLElement {
  const args = parseJsonSafe(item.tool_args);

  const card = document.createElement('div');
  card.className = `tool-custom-card tool-generic-card status-${item.tool_status}`;

  card.addEventListener('wheel', (e) => e.stopPropagation(), { passive: true });

  // Render parameter chips
  let chipsHtml = '';
  const entries = Object.entries(args);
  if (entries.length > 0) {
    entries.slice(0, 4).forEach(([key, val]) => {
      const valStr = typeof val === 'object' ? JSON.stringify(val) : String(val);
      chipsHtml += `<span class="tool-arg-chip"><strong>${escapeHtml(key)}:</strong> ${escapeHtml(valStr)}</span>`;
    });
  }

  const header = document.createElement('div');
  header.className = 'tool-custom-header';

  const leftGroup = document.createElement('div');
  leftGroup.className = 'tool-header-left';
  leftGroup.innerHTML = `
    <div class="tool-args-chips-container">${chipsHtml || `<span class="tool-arg-chip">${escapeHtml(item.tool_name)}</span>`}</div>
  `;
  header.appendChild(leftGroup);

  const rightGroup = document.createElement('div');
  rightGroup.className = 'tool-header-right';
  rightGroup.appendChild(createStatusSvgIcon(item.tool_status));

  const copyBtn = document.createElement('button');
  copyBtn.className = 'diff-copy-btn';
  copyBtn.type = 'button';
  copyBtn.title = 'Copy output';
  copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
  copyBtn.addEventListener('click', async (e) => {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(item.tool_result || '');
      copyBtn.classList.add('copied');
      copyBtn.innerHTML = '<span class="ui-icon icon-diff-check"></span>';
      setTimeout(() => {
        copyBtn.classList.remove('copied');
        copyBtn.innerHTML = '<span class="ui-icon icon-diff-copy"></span>';
      }, 2000);
    } catch {}
  });
  rightGroup.appendChild(copyBtn);
  header.appendChild(rightGroup);
  card.appendChild(header);

  const genericBody = document.createElement('div');
  genericBody.className = 'tool-generic-body';
  genericBody.innerHTML = `<pre class="tool-generic-pre"><code class="language-json">${escapeHtml(item.tool_result || (item.tool_status === 'running' ? 'Tool is executing...' : '(no output)'))}</code></pre>`;
  card.appendChild(genericBody);

  return card;
}
