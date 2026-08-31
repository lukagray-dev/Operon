// ============================================================================
// Myers & Unified Diff Computation Utilities for Operon Tool Cards
//
// Hey friend! Welcome to the diff utilities module!
// This file contains a pure TypeScript implementation of Eugene W. Myers'
// classic $O(ND)$ Difference Algorithm (the same algorithm Git uses under the
// hood to compare files line-by-line).
//
// Why write our own Myers algorithm instead of importing an npm package?
// 1. Zero external runtime dependencies: Keeps our Tauri app bundle tiny and super fast.
// 2. Predictable performance: Runs deterministically in microseconds on typical code edits.
// 3. GitHub-accurate output: Computes unified hunks with unchanged context lines,
//    deleted lines ('-'), and added lines ('+') complete with precise old and new
//    line numbers and @@ hunk headers matching GitHub's diff viewer.
// ============================================================================

/**
 * Represents a single line in a diff view.
 */
export interface DiffLine {
  /** Line type: 'del' for deleted (-), 'add' for added (+), 'context' for unchanged */
  type: 'del' | 'add' | 'context';
  /** The text content of this line */
  content: string;
  /** Line number in the original/old file (if applicable) */
  oldLineNum?: number;
  /** Line number in the modified/new file (if applicable) */
  newLineNum?: number;
}

/**
 * Represents a unified diff hunk (a contiguous block of changes with surrounding context).
 */
export interface DiffHunk {
  /** Hunk header string, e.g. "@@ -49,2 +49,1 @@" */
  header: string;
  /** Starting line number in the old file */
  oldStart: number;
  /** Number of lines from the old file in this hunk */
  oldCount: number;
  /** Starting line number in the new file */
  newStart: number;
  /** Number of lines from the new file in this hunk */
  newCount: number;
  /** Context header label if available (e.g. function or section name) */
  contextTitle?: string;
  /** The sequence of lines (context, deletions, additions) within this hunk */
  lines: DiffLine[];
}

/**
 * Summary statistics of total insertions and deletions across all hunks.
 */
export interface DiffStats {
  /** Total number of added lines (+) */
  insertions: number;
  /** Total number of deleted lines (-) */
  deletions: number;
}

/**
 * Complete diff result containing all calculated hunks and aggregate stats.
 */
export interface DiffResult {
  /** Array of unified hunks ready for rendering */
  hunks: DiffHunk[];
  /** Aggregate line change counters */
  stats: DiffStats;
}

// ----------------------------------------------------------------------------
// Section 1: Myers Difference Algorithm Implementation
// ----------------------------------------------------------------------------

/**
 * Internal edit operation produced by the Myers algorithm.
 */
interface RawEditOp {
  type: 'insert' | 'delete' | 'equal';
  text: string;
}

/**
 * Compares two arrays of strings (lines) using the classic Myers diff algorithm
 * and returns the shortest edit script (SES).
 *
 * How Myers algorithm works (like explaining to a newbie friend):
 * Imagine a grid where horizontal movements represent deleting a line from array A,
 * vertical movements represent inserting a line from array B, and diagonal
 * movements represent lines that are identical in both files (free moves!).
 * The algorithm searches for the path with the fewest non-diagonal steps ($D$).
 *
 * @param a - Original lines of text
 * @param b - Modified lines of text
 * @returns Array of raw edit operations (equal, delete, insert)
 */
export function myersDiffLines(a: string[], b: string[]): RawEditOp[] {
  const n = a.length;
  const m = b.length;
  const max = n + m;

  // Shortcut 1: If old text is empty, all lines are insertions
  if (n === 0) {
    return b.map((line) => ({ type: 'insert', text: line }));
  }

  // Shortcut 2: If new text is empty, all lines are deletions
  if (m === 0) {
    return a.map((line) => ({ type: 'delete', text: line }));
  }

  // V-array stores the furthest reaching point (x-coordinate) on each diagonal k
  // k is defined as (x - y), ranging from -max to +max
  const vOffset = max;
  const v = new Int32Array(2 * max + 1);
  // Track trace history for backtracking the path
  const trace: Int32Array[] = [];

  for (let d = 0; d <= max; d++) {
    const vCopy = new Int32Array(v);
    trace.push(vCopy);

    for (let k = -d; k <= d; k += 2) {
      let x: number;

      // Decide whether to move down from diagonal k-1 or right from diagonal k+1
      if (k === -d || (k !== d && v[k - 1 + vOffset] < v[k + 1 + vOffset])) {
        x = v[k + 1 + vOffset]; // Down (insertion)
      } else {
        x = v[k - 1 + vOffset] + 1; // Right (deletion)
      }

      let y = x - k;

      // Slide along identical lines (diagonal movement)
      while (x < n && y < m && a[x] === b[y]) {
        x++;
        y++;
      }

      v[k + vOffset] = x;

      // If we reached the bottom-right corner (n, m), we found the optimal diff!
      if (x >= n && y >= m) {
        return backtrackMyersPath(trace, a, b, d, max);
      }
    }
  }

  // Fallback (should rarely occur): delete all old, insert all new
  const fallback: RawEditOp[] = [];
  for (const line of a) fallback.push({ type: 'delete', text: line });
  for (const line of b) fallback.push({ type: 'insert', text: line });
  return fallback;
}

/**
 * Backtracks the trace history from (N, M) back to (0, 0) to construct the edit script.
 */
function backtrackMyersPath(
  trace: Int32Array[],
  a: string[],
  b: string[],
  d: number,
  max: number
): RawEditOp[] {
  const ops: RawEditOp[] = [];
  let x = a.length;
  let y = b.length;

  for (let step = d; step > 0; step--) {
    const v = trace[step];
    const k = x - y;

    let prevK: number;
    if (k === -step || (k !== step && v[k - 1 + max] < v[k + 1 + max])) {
      prevK = k + 1;
    } else {
      prevK = k - 1;
    }

    const prevX = v[prevK + max];
    const prevY = prevX - prevK;

    // Add diagonal identical lines
    while (x > prevX && y > prevY) {
      x--;
      y--;
      ops.unshift({ type: 'equal', text: a[x] });
    }

    if (x === prevX) {
      // Insertion
      y--;
      ops.unshift({ type: 'insert', text: b[y] });
    } else {
      // Deletion
      x--;
      ops.unshift({ type: 'delete', text: a[x] });
    }
  }

  // Add any remaining diagonal matching lines at the top
  while (x > 0 && y > 0) {
    x--;
    y--;
    ops.unshift({ type: 'equal', text: a[x] });
  }

  return ops;
}

// ----------------------------------------------------------------------------
// Section 2: Hunk Grouping & Formatting
// ----------------------------------------------------------------------------

/**
 * Converts raw edit operations into unified diff hunks with surrounding context lines.
 *
 * @param ops - Raw edit operations from Myers algorithm
 * @param baseOldLine - Base line number in the original file (defaults to 1)
 * @param baseNewLine - Base line number in the new file (defaults to 1)
 * @param contextLines - Number of context lines to keep before and after changes (default: 3)
 */
export function buildUnifiedHunks(
  ops: RawEditOp[],
  baseOldLine = 1,
  baseNewLine = 1,
  contextLines = 3
): DiffResult {
  let oldLineCounter = baseOldLine;
  let newLineCounter = baseNewLine;

  // 1. Tag each operation with its calculated old and new line numbers
  const annotatedLines: DiffLine[] = [];
  let totalInsertions = 0;
  let totalDeletions = 0;

  for (const op of ops) {
    if (op.type === 'equal') {
      annotatedLines.push({
        type: 'context',
        content: op.text,
        oldLineNum: oldLineCounter++,
        newLineNum: newLineCounter++,
      });
    } else if (op.type === 'delete') {
      annotatedLines.push({
        type: 'del',
        content: op.text,
        oldLineNum: oldLineCounter++,
      });
      totalDeletions++;
    } else if (op.type === 'insert') {
      annotatedLines.push({
        type: 'add',
        content: op.text,
        newLineNum: newLineCounter++,
      });
      totalInsertions++;
    }
  }

  // 2. Identify indices of all change lines (additions or deletions)
  const changeIndices: number[] = [];
  for (let i = 0; i < annotatedLines.length; i++) {
    if (annotatedLines[i].type !== 'context') {
      changeIndices.push(i);
    }
  }

  // If there are no changes at all (identical strings), return single clean context hunk or empty
  if (changeIndices.length === 0) {
    return {
      hunks: [
        {
          header: `@@ -${baseOldLine},${annotatedLines.length} +${baseNewLine},${annotatedLines.length} @@`,
          oldStart: baseOldLine,
          oldCount: annotatedLines.length,
          newStart: baseNewLine,
          newCount: annotatedLines.length,
          lines: annotatedLines,
        },
      ],
      stats: { insertions: 0, deletions: 0 },
    };
  }

  // 3. Cluster changes into hunks separated by unchanged context regions
  interface HunkRange {
    startIndex: number;
    endIndex: number;
  }

  const ranges: HunkRange[] = [];
  let currentRange: HunkRange = {
    startIndex: Math.max(0, changeIndices[0] - contextLines),
    endIndex: Math.min(annotatedLines.length - 1, changeIndices[0] + contextLines),
  };

  for (let i = 1; i < changeIndices.length; i++) {
    const idx = changeIndices[i];
    const neededStart = Math.max(0, idx - contextLines);
    const neededEnd = Math.min(annotatedLines.length - 1, idx + contextLines);

    // If this change overlaps or is within 2 * contextLines of the current hunk, merge them!
    if (neededStart <= currentRange.endIndex + 1) {
      currentRange.endIndex = Math.max(currentRange.endIndex, neededEnd);
    } else {
      ranges.push(currentRange);
      currentRange = { startIndex: neededStart, endIndex: neededEnd };
    }
  }
  ranges.push(currentRange);

  // 4. Construct formatted DiffHunk objects from ranges
  const hunks: DiffHunk[] = ranges.map((r) => {
    const lines = annotatedLines.slice(r.startIndex, r.endIndex + 1);

    // Calculate line counts for old and new files
    let oldCount = 0;
    let newCount = 0;
    let oldStart = baseOldLine;
    let newStart = baseNewLine;
    let firstOldAssigned = false;
    let firstNewAssigned = false;

    for (const l of lines) {
      if (l.type === 'context') {
        oldCount++;
        newCount++;
        if (!firstOldAssigned && l.oldLineNum !== undefined) {
          oldStart = l.oldLineNum;
          firstOldAssigned = true;
        }
        if (!firstNewAssigned && l.newLineNum !== undefined) {
          newStart = l.newLineNum;
          firstNewAssigned = true;
        }
      } else if (l.type === 'del') {
        oldCount++;
        if (!firstOldAssigned && l.oldLineNum !== undefined) {
          oldStart = l.oldLineNum;
          firstOldAssigned = true;
        }
      } else if (l.type === 'add') {
        newCount++;
        if (!firstNewAssigned && l.newLineNum !== undefined) {
          newStart = l.newLineNum;
          firstNewAssigned = true;
        }
      }
    }

    const header = `@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`;
    return {
      header,
      oldStart,
      oldCount,
      newStart,
      newCount,
      lines,
    };
  });

  return {
    hunks,
    stats: {
      insertions: totalInsertions,
      deletions: totalDeletions,
    },
  };
}

/**
 * Computes a unified diff between two text strings.
 *
 * @param oldText - Original string before modification
 * @param newText - Modified string after modification
 * @param startLine - Optional 1-based start line number in original file
 * @param contextLines - Number of surrounding context lines to retain
 */
export function computeDiffBetweenTexts(
  oldText: string,
  newText: string,
  startLine = 1,
  contextLines = 3
): DiffResult {
  const oldLines = oldText.length > 0 ? oldText.split(/\r?\n/) : [];
  const newLines = newText.length > 0 ? newText.split(/\r?\n/) : [];

  const ops = myersDiffLines(oldLines, newLines);
  return buildUnifiedHunks(ops, startLine, startLine, contextLines);
}

/**
 * Generates a full creation / write diff (all lines marked as additions '+').
 *
 * @param content - Content of the newly created or written file
 * @param startLine - Starting line number (defaults to 1)
 */
export function computeWriteDiff(content: string, startLine = 1): DiffResult {
  const lines = content.length > 0 ? content.split(/\r?\n/) : [];
  const diffLines: DiffLine[] = lines.map((text, idx) => ({
    type: 'add',
    content: text,
    newLineNum: startLine + idx,
  }));

  const count = diffLines.length;
  const header = `@@ -0,0 +${startLine},${count} @@`;

  return {
    hunks: [
      {
        header,
        oldStart: 0,
        oldCount: 0,
        newStart: startLine,
        newCount: count,
        lines: diffLines,
      },
    ],
    stats: {
      insertions: count,
      deletions: 0,
    },
  };
}

/**
 * Generates an append diff (appended lines marked as additions '+').
 *
 * @param appendText - Text content appended to the file
 * @param approximateStartLine - Estimated line number where append begins (e.g. 1 or N)
 */
export function computeAppendDiff(appendText: string, approximateStartLine = 1): DiffResult {
  const lines = appendText.length > 0 ? appendText.split(/\r?\n/) : [];
  const diffLines: DiffLine[] = lines.map((text, idx) => ({
    type: 'add',
    content: text,
    newLineNum: approximateStartLine + idx,
  }));

  const count = diffLines.length;
  const header = `@@ +${approximateStartLine},${count} @@ (Appended)`;

  return {
    hunks: [
      {
        header,
        oldStart: approximateStartLine,
        oldCount: 0,
        newStart: approximateStartLine,
        newCount: count,
        lines: diffLines,
      },
    ],
    stats: {
      insertions: count,
      deletions: 0,
    },
  };
}

// ----------------------------------------------------------------------------
// Section 3: Syntax Highlighting Language Resolution
// ----------------------------------------------------------------------------

/**
 * Maps a file path or file extension to a highlight.js compatible language name.
 *
 * @param filePath - Path or filename (e.g. "src/main.rs", "App.tsx", "styles.css")
 * @returns Language identifier recognized by highlight.js (e.g. "rust", "typescript")
 */
export function detectLanguageFromPath(filePath: string): string {
  if (!filePath) return 'text';

  const extMatch = filePath.match(/\.([a-zA-Z0-9_-]+)$/);
  if (!extMatch) {
    const filename = filePath.split(/[/\\]/).pop()?.toLowerCase() || '';
    if (filename === 'dockerfile') return 'dockerfile';
    if (filename === 'makefile') return 'makefile';
    if (filename === 'cargo.lock' || filename === 'cargo.toml') return 'toml';
    if (filename === 'package.json') return 'json';
    return 'text';
  }

  const ext = extMatch[1].toLowerCase();
  switch (ext) {
    case 'rs':
      return 'rust';
    case 'ts':
    case 'mts':
    case 'cts':
      return 'typescript';
    case 'tsx':
      return 'typescript';
    case 'js':
    case 'mjs':
    case 'cjs':
      return 'javascript';
    case 'jsx':
      return 'javascript';
    case 'py':
    case 'pyw':
      return 'python';
    case 'json':
    case 'jsonc':
      return 'json';
    case 'toml':
      return 'toml';
    case 'yaml':
    case 'yml':
      return 'yaml';
    case 'css':
      return 'css';
    case 'scss':
    case 'sass':
      return 'scss';
    case 'html':
    case 'htm':
      return 'html';
    case 'xml':
    case 'svg':
      return 'xml';
    case 'md':
    case 'markdown':
      return 'markdown';
    case 'sh':
    case 'bash':
    case 'zsh':
      return 'bash';
    case 'ps1':
    case 'psm1':
      return 'powershell';
    case 'c':
    case 'h':
      return 'c';
    case 'cpp':
    case 'hpp':
    case 'cc':
    case 'cxx':
      return 'cpp';
    case 'cs':
      return 'csharp';
    case 'go':
      return 'go';
    case 'java':
      return 'java';
    case 'kt':
    case 'kts':
      return 'kotlin';
    case 'swift':
      return 'swift';
    case 'sql':
      return 'sql';
    case 'php':
      return 'php';
    case 'rb':
      return 'ruby';
    case 'lua':
      return 'lua';
    default:
      return 'text';
  }
}

