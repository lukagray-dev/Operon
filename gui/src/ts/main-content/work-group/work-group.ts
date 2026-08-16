// Assistant WorkGroup and Tool Execution Timeline DOM Renderer
//
// 1:1 visual and behavioral redesign inspired by the Slint GUI WorkGroup component:
// - Header with Composing Thinking Orb (while active), shining summary text ("Working..." or "Thought • 2 tool calls | Worked for 3s."), and trailing chevron.
// - Expandable timeline with a 1px vertical spine connecting 14x14 dark-masked checkpoint tool/thinking icons.
// - Sub-tool collapsible items with inline ToolDetailBody displaying BOTH input arguments and execution output.
// - In-place DOM synchronization (syncWorkGroupElement) preserving CSS shine animations and click responsiveness during high-frequency token streams.

import { getCachedAppearance } from '../markdown/markdown.js';
import { ThinkingOrbRenderer } from './orb.js';
import type { WorkGroupData, WorkGroupItem } from './types.js';

function getOrbState(idx: number): 'composing' | 'shaping' | 'working' | 'connecting' {
  switch (idx) {
    case 1:
      return 'shaping';
    case 2:
      return 'working';
    case 3:
      return 'connecting';
    default:
      return 'composing';
  }
}

function getOrbSpeedMultiplier(idx: number): number {
  switch (idx) {
    case 0:
      return 1.5;
    case 2:
      return 4.5;
    default:
      return 3.0;
  }
}

/**
 * Checks if a tool name belongs to directory listing.
 */
export function isListTool(name: string): boolean {
  return name === 'ls' || name === 'list_dir';
}

/**
 * Checks if a tool name belongs to file reading.
 */
export function isReadTool(name: string): boolean {
  return name === 'read' || name === 'view_file' || name === 'read_file';
}

/**
 * Checks if a tool name belongs to file creation/editing.
 */
export function isEditTool(name: string): boolean {
  return (
    name === 'write' ||
    name === 'edit' ||
    name === 'append' ||
    name === 'write_to_file' ||
    name === 'replace_file_content' ||
    name === 'multi_replace_file_content'
  );
}

/**
 * Checks if a tool name belongs to codebase search or grep.
 */
export function isSearchTool(name: string): boolean {
  return (
    name === 'search' ||
    name === 'web_search' ||
    name === 'search_web' ||
    name === 'grep' ||
    name === 'grep_search'
  );
}

/**
 * Checks if a tool name belongs to web fetching or scraping.
 */
export function isWebTool(name: string): boolean {
  return name === 'web' || name === 'web_fetch' || name === 'read_url_content' || name === 'fetch';
}

/**
 * Returns the appropriate CSS mask icon class for a tool.
 */
export function getItemIconClass(name: string): string {
  if (isListTool(name)) return 'icon-tool-list';
  if (isReadTool(name)) return 'icon-tool-read';
  if (isEditTool(name)) return 'icon-tool-edit';
  if (isSearchTool(name)) return 'icon-tool-search';
  if (isWebTool(name)) return 'icon-tool-web';
  return 'icon-tool-general';
}

/**
 * Returns the icon tint color based on tool type and completion status.
 */
export function getItemIconColor(name: string, status: string): string {
  if (status === 'failed') return '#ef9a9a';
  if (status === 'completed' && isEditTool(name)) return '#81c784';
  return '#999999';
}

/**
 * Returns the icon opacity based on tool type.
 */
export function getItemIconOpacity(name: string): string {
  if (isEditTool(name)) return '0.82';
  return '0.78';
}

/**
 * Generates a human-friendly descriptive title for tool actions.
 */
export function getToolFriendlyTitle(name: string, argsJson: string): string {
  let parsedArgs: Record<string, unknown> = {};
  try {
    if (argsJson) parsedArgs = JSON.parse(argsJson);
  } catch {
    // Fallback on raw string
  }

  const rawPath = (parsedArgs.path ||
    parsedArgs.TargetFile ||
    parsedArgs.DirectoryPath ||
    parsedArgs.AbsolutePath ||
    '') as string;
  const shortPath = rawPath ? rawPath.split(/[/\\]/).pop() || rawPath : '';

  if (isReadTool(name)) {
    return shortPath ? `Reading ${shortPath}` : 'Reading file';
  }
  if (isEditTool(name)) {
    return shortPath ? `Editing ${shortPath}` : 'Editing file';
  }
  if (isListTool(name)) {
    return shortPath ? `Listing directory ${shortPath}` : 'Listing directory';
  }
  if (name === 'grep_search' || name === 'grep') {
    return parsedArgs.Query ? `Searching "${parsedArgs.Query}"` : 'Searching codebase';
  }
  if (name === 'search_web' || name === 'web_search') {
    return parsedArgs.query ? `Web search: "${parsedArgs.query}"` : 'Web search';
  }
  if (name === 'read_url_content' || name === 'web_fetch') {
    return parsedArgs.Url ? `Fetching ${parsedArgs.Url}` : 'Fetching URL';
  }
  if (name === 'run_command' || name === 'bash' || name === 'exec') {
    return parsedArgs.CommandLine ? `Running: ${parsedArgs.CommandLine}` : 'Running command';
  }

  return `Running ${name}`;
}

/**
 * Builds the summary label for a completed work activity.
 */
export function getWorkGroupSummaryText(items: WorkGroupItem[]): string {
  const toolCount = items.filter((i) => i.kind === 'tool').length;
  const hasThinking = items.some((i) => i.kind === 'thinking');

  if (hasThinking && toolCount > 0) {
    return `Thought • ${toolCount} tool call${toolCount > 1 ? 's' : ''}`;
  }
  if (hasThinking) {
    return 'Thought process';
  }
  if (toolCount > 0) {
    return `${toolCount} tool call${toolCount > 1 ? 's' : ''}`;
  }
  return 'Activity';
}

/**
 * Updates the 1px vertical spine connector line based on row index and count.
 */
function updateSpineLine(row: HTMLElement, idx: number, totalCount: number): void {
  const line = row.querySelector('.work-group-timeline-line');
  if (!line) return;

  if (totalCount === 1) {
    line.className = 'work-group-timeline-line single';
  } else if (idx === 0) {
    line.className = 'work-group-timeline-line first';
  } else if (idx === totalCount - 1) {
    line.className = 'work-group-timeline-line last';
  } else {
    line.className = 'work-group-timeline-line middle';
  }
}

/**
 * Formats JSON text for display in ToolDetailBody.
 */
function formatJsonSafe(raw: string): string {
  try {
    if (raw && (raw.trim().startsWith('{') || raw.trim().startsWith('['))) {
      const parsed = JSON.parse(raw);
      return JSON.stringify(parsed, null, 2);
    }
  } catch {
    // Keep raw string if parse fails
  }
  return raw;
}

/**
 * Creates the collapsible ToolDetailBody (args and output).
 */
function createToolDetailBody(item: WorkGroupItem & { kind: 'tool' }): HTMLElement {
  const detailBody = document.createElement('div');
  detailBody.className = 'tool-detail-body';

  detailBody.addEventListener(
    'wheel',
    (e) => {
      e.stopPropagation();
    },
    { passive: true }
  );

  // 1. Tool Input Arguments Section
  const argsEl = document.createElement('div');
  argsEl.className = 'tool-args-text';
  const formattedArgs = formatJsonSafe(item.tool_args);
  argsEl.textContent = `args: ${formattedArgs || '{}'}`;
  detailBody.appendChild(argsEl);

  // 2. Horizontal Divider separating Args and Output
  const divider = document.createElement('div');
  divider.className = 'tool-detail-divider';
  detailBody.appendChild(divider);

  // 3. Tool Execution Output Result Section
  const resultEl = document.createElement('div');
  resultEl.className = `tool-result-text ${item.tool_status === 'failed' ? 'failed' : ''}`;
  const formattedResult = formatJsonSafe(item.tool_result);
  if (formattedResult && formattedResult.trim().length > 0) {
    resultEl.textContent = `output: ${formattedResult}`;
  } else if (item.tool_status === 'running') {
    resultEl.textContent = 'output: Tool is executing...';
  } else {
    resultEl.textContent = 'output: (no output)';
  }
  detailBody.appendChild(resultEl);

  return detailBody;
}

/**
 * In-place updater for ToolDetailBody.
 */
function updateToolDetailBody(detailBody: HTMLElement, item: WorkGroupItem & { kind: 'tool' }): void {
  const argsEl = detailBody.querySelector('.tool-args-text');
  if (argsEl) {
    const formattedArgs = formatJsonSafe(item.tool_args);
    const expectedArgs = `args: ${formattedArgs || '{}'}`;
    if (argsEl.textContent !== expectedArgs) argsEl.textContent = expectedArgs;
  }

  const resultEl = detailBody.querySelector('.tool-result-text');
  if (resultEl) {
    resultEl.className = `tool-result-text ${item.tool_status === 'failed' ? 'failed' : ''}`;
    const formattedResult = formatJsonSafe(item.tool_result);
    let expectedRes = 'output: (no output)';
    if (formattedResult && formattedResult.trim().length > 0) {
      expectedRes = `output: ${formattedResult}`;
    } else if (item.tool_status === 'running') {
      expectedRes = 'output: Tool is executing...';
    }
    if (resultEl.textContent !== expectedRes) resultEl.textContent = expectedRes;
  }
}

/**
 * Creates a brand new timeline row element.
 */
function renderTimelineItemRow(
  item: WorkGroupItem,
  idx: number,
  totalCount: number,
  onToggleItem: (itemIdx: number) => void
): HTMLElement {
  const row = document.createElement('div');
  row.className = 'work-group-timeline-row';
  row.setAttribute('data-item-kind', item.kind);
  row.setAttribute('data-item-idx', String(idx));

  // Left Column: 20px timeline spine
  const spine = document.createElement('div');
  spine.className = 'work-group-timeline-spine';

  const line = document.createElement('div');
  spine.appendChild(line);

  const mask = document.createElement('div');
  mask.className = 'work-group-timeline-mask';
  spine.appendChild(mask);

  const icon = document.createElement('span');
  if (item.kind === 'thinking') {
    icon.className = 'ui-icon icon-tool-thinking work-group-timeline-icon';
    icon.style.backgroundColor = '#999999';
    icon.style.opacity = '0.72';
  } else {
    const iconClass = getItemIconClass(item.tool_name);
    const iconColor = getItemIconColor(item.tool_name, item.tool_status);
    const iconOpacity = getItemIconOpacity(item.tool_name);
    icon.className = `ui-icon ${iconClass} work-group-timeline-icon`;
    icon.style.backgroundColor = iconColor;
    icon.style.opacity = iconOpacity;
  }
  spine.appendChild(icon);
  row.appendChild(spine);

  // Right Column: Item Content
  const content = document.createElement('div');
  content.className = 'work-group-timeline-content';

  if (item.kind === 'thinking') {
    const textBlock = document.createElement('div');
    textBlock.className = 'work-group-thinking-text';
    textBlock.textContent = item.thinking_text;
    content.appendChild(textBlock);
  } else if (item.kind === 'tool') {
    const toolHeader = document.createElement('div');
    toolHeader.className = 'work-group-tool-header';

    const toolTitle = document.createElement('span');
    toolTitle.className = `work-group-tool-title ${item.tool_status === 'running' ? 'running' : ''}`;
    toolTitle.textContent = item.tool_title || `Running ${item.tool_name}`;

    const subChevron = document.createElement('span');
    subChevron.className = `ui-icon icon-tool-chevron work-group-tool-chevron ${
      item.is_expanded ? 'expanded' : ''
    }`;

    toolHeader.appendChild(toolTitle);
    toolHeader.appendChild(subChevron);

    toolHeader.addEventListener('click', () => {
      const currentIdx = parseInt(row.getAttribute('data-item-idx') || String(idx), 10);
      onToggleItem(currentIdx);
    });

    content.appendChild(toolHeader);

    if (item.is_expanded) {
      content.appendChild(createToolDetailBody(item));
    }
  }

  row.appendChild(content);
  updateSpineLine(row, idx, totalCount);
  return row;
}

/**
 * Updates an existing timeline row in-place.
 */
function updateTimelineItemRow(
  row: HTMLElement,
  item: WorkGroupItem,
  idx: number,
  totalCount: number
): void {
  row.setAttribute('data-item-idx', String(idx));
  updateSpineLine(row, idx, totalCount);

  if (item.kind === 'thinking') {
    const textBlock = row.querySelector('.work-group-thinking-text');
    if (textBlock && textBlock.textContent !== item.thinking_text) {
      textBlock.textContent = item.thinking_text;
    }
  } else if (item.kind === 'tool') {
    // 1. Tool title & running shine
    const toolTitle = row.querySelector('.work-group-tool-title');
    if (toolTitle) {
      const isRunning = item.tool_status === 'running';
      toolTitle.classList.toggle('running', isRunning);
      const expectedTitle = item.tool_title || `Running ${item.tool_name}`;
      if (toolTitle.textContent !== expectedTitle) {
        toolTitle.textContent = expectedTitle;
      }
    }

    // 2. Icon color
    const icon = row.querySelector('.work-group-timeline-icon') as HTMLElement | null;
    if (icon) {
      icon.style.backgroundColor = getItemIconColor(item.tool_name, item.tool_status);
    }

    // 3. Chevron rotation
    const subChevron = row.querySelector('.work-group-tool-chevron');
    if (subChevron) {
      subChevron.classList.toggle('expanded', item.is_expanded);
    }

    // 4. Detail body
    const content = row.querySelector('.work-group-timeline-content');
    const existingBody = row.querySelector('.tool-detail-body') as HTMLElement | null;

    if (item.is_expanded) {
      if (!existingBody && content) {
        content.appendChild(createToolDetailBody(item));
      } else if (existingBody) {
        updateToolDetailBody(existingBody, item);
      }
    } else if (existingBody) {
      existingBody.remove();
    }
  }
}

/**
 * Synchronizes a WorkGroup DOM element in-place, preserving CSS animations and click responsiveness.
 */
export function syncWorkGroupElement(
  existingContainer: HTMLElement | null,
  data: WorkGroupData,
  onToggleExpand: () => void,
  onToggleItem: (itemIdx: number) => void,
  existingOrbRenderer?: ThinkingOrbRenderer | null
): { element: HTMLElement; orbRenderer: ThinkingOrbRenderer | null } {
  let orbRenderer = existingOrbRenderer || null;

  // If no existing container, construct full element from scratch
  if (!existingContainer) {
    const container = document.createElement('div');
    container.className = `work-group-container ${data.is_expanded ? 'expanded' : ''}`;

    // Header
    const header = document.createElement('div');
    header.className = 'work-group-header';

    if (data.is_active) {
      const appearance = getCachedAppearance();
      if (appearance.show_live_orb !== false) {
        const orbContainer = document.createElement('div');
        orbContainer.className = 'work-group-orb-container';

        const canvas = document.createElement('canvas');
        canvas.className = 'work-group-orb-canvas';
        orbContainer.appendChild(canvas);
        header.appendChild(orbContainer);

        const state = getOrbState(appearance.selected_thinking_orb);
        const speed = getOrbSpeedMultiplier(appearance.orb_speed);

        if (orbRenderer) {
          orbRenderer.attachCanvas(canvas);
          orbRenderer.setState(state);
          orbRenderer.setSpeed(speed);
          orbRenderer.start();
        } else {
          try {
            orbRenderer = new ThinkingOrbRenderer(canvas, {
              state,
              size: 20,
              speed,
              dark: true,
            });
            orbRenderer.start();
          } catch (e) {
            console.warn('[WorkGroup] Failed to initialize ThinkingOrbRenderer:', e);
          }
        }
      }
    }

    const summary = document.createElement('span');
    summary.className = `work-group-summary-text ${data.is_active ? 'active' : ''}`;
    if (data.is_active) {
      summary.textContent = 'Working...';
    } else {
      const baseSummary = getWorkGroupSummaryText(data.items);
      summary.textContent = `${baseSummary} | Worked for ${data.elapsed_secs}s.`;
    }
    header.appendChild(summary);

    const chevron = document.createElement('span');
    chevron.className = 'ui-icon icon-tool-chevron work-group-chevron';
    header.appendChild(chevron);

    header.addEventListener('click', () => {
      onToggleExpand();
    });
    container.appendChild(header);

    // Timeline
    const timeline = document.createElement('div');
    timeline.className = 'work-group-timeline';
    const totalCount = data.items.length;
    data.items.forEach((item, idx) => {
      timeline.appendChild(renderTimelineItemRow(item, idx, totalCount, onToggleItem));
    });
    container.appendChild(timeline);

    return { element: container, orbRenderer };
  }

  // ===== IN-PLACE RECONCILIATION FOR EXISTING DOM CONTAINER =====
  existingContainer.classList.toggle('expanded', data.is_expanded);

  const header = existingContainer.querySelector('.work-group-header') as HTMLElement | null;
  const summary = existingContainer.querySelector('.work-group-summary-text') as HTMLElement | null;

  // 1. Thinking Orb Lifecycle in header
  if (data.is_active) {
    let orbContainer = existingContainer.querySelector('.work-group-orb-container') as HTMLElement | null;
    if (!orbContainer && header) {
      const appearance = getCachedAppearance();
      if (appearance.show_live_orb !== false) {
        orbContainer = document.createElement('div');
        orbContainer.className = 'work-group-orb-container';
        const canvas = document.createElement('canvas');
        canvas.className = 'work-group-orb-canvas';
        orbContainer.appendChild(canvas);
        header.insertBefore(orbContainer, summary || header.firstChild);

        const state = getOrbState(appearance.selected_thinking_orb);
        const speed = getOrbSpeedMultiplier(appearance.orb_speed);

        if (orbRenderer) {
          orbRenderer.attachCanvas(canvas);
          orbRenderer.setState(state);
          orbRenderer.setSpeed(speed);
          orbRenderer.start();
        } else {
          try {
            orbRenderer = new ThinkingOrbRenderer(canvas, {
              state,
              size: 20,
              speed,
              dark: true,
            });
            orbRenderer.start();
          } catch (e) {
            console.warn('[WorkGroup] Failed to initialize ThinkingOrbRenderer:', e);
          }
        }
      }
    }
  } else {
    const orbContainer = existingContainer.querySelector('.work-group-orb-container');
    if (orbContainer) {
      orbContainer.remove();
    }
    if (orbRenderer) {
      orbRenderer.destroy();
      orbRenderer = null;
    }
  }

  // 2. Summary Text & Active Shine Sweep
  if (summary) {
    summary.classList.toggle('active', data.is_active);
    if (data.is_active) {
      if (summary.textContent !== 'Working...') {
        summary.textContent = 'Working...';
      }
    } else {
      const baseSummary = getWorkGroupSummaryText(data.items);
      const finalSummary = `${baseSummary} | Worked for ${data.elapsed_secs}s.`;
      if (summary.textContent !== finalSummary) {
        summary.textContent = finalSummary;
      }
    }
  }

  // 3. Timeline Items In-Place Update
  let timeline = existingContainer.querySelector('.work-group-timeline') as HTMLElement | null;
  if (!timeline) {
    timeline = document.createElement('div');
    timeline.className = 'work-group-timeline';
    existingContainer.appendChild(timeline);
  }

  const totalCount = data.items.length;
  const existingRows = Array.from(timeline.children) as HTMLElement[];

  // Remove trailing excess rows
  while (existingRows.length > totalCount) {
    const extra = existingRows.pop();
    extra?.remove();
  }

  data.items.forEach((item, idx) => {
    const existingRow = existingRows[idx];
    if (existingRow && existingRow.getAttribute('data-item-kind') === item.kind) {
      updateTimelineItemRow(existingRow, item, idx, totalCount);
    } else if (existingRow) {
      const newRow = renderTimelineItemRow(item, idx, totalCount, onToggleItem);
      existingRow.replaceWith(newRow);
    } else {
      const newRow = renderTimelineItemRow(item, idx, totalCount, onToggleItem);
      timeline.appendChild(newRow);
    }
  });

  // Ensure spine lines updated for all rows if count changed
  timeline.querySelectorAll<HTMLElement>('.work-group-timeline-row').forEach((row, rIdx) => {
    updateSpineLine(row, rIdx, totalCount);
  });

  return { element: existingContainer, orbRenderer };
}

/**
 * Renders the full WorkGroup DOM element (backwards compatibility).
 */
export function renderWorkGroupElement(
  data: WorkGroupData,
  onToggleExpand: () => void,
  onToggleItem: (itemIdx: number) => void,
  existingOrbRenderer?: ThinkingOrbRenderer | null
): { element: HTMLElement; orbRenderer: ThinkingOrbRenderer | null } {
  return syncWorkGroupElement(null, data, onToggleExpand, onToggleItem, existingOrbRenderer);
}
