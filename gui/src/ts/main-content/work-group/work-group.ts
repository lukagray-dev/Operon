// Assistant WorkGroup and Tool Execution Timeline DOM Renderer
//
// 1:1 visual and behavioral redesign inspired by the Slint GUI WorkGroup component:
// - Header with Composing Thinking Orb (while active), summary text ("Working..." or "Thought • 2 tool calls | Worked for 3s."), and trailing chevron.
// - Expandable timeline with a 1px vertical spine connecting 14x14 dark-masked checkpoint tool/thinking icons.
// - Sub-tool collapsible items with inline ToolDetailBody displaying BOTH input arguments and execution output.

import { ThinkingOrbRenderer } from './orb.js';
import type { WorkGroupData, WorkGroupItem } from './types.js';

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
 * Renders the full WorkGroup DOM element matching the Slint design.
 */
export function renderWorkGroupElement(
  data: WorkGroupData,
  onToggleExpand: () => void,
  onToggleItem: (itemIdx: number) => void,
  existingOrbRenderer?: ThinkingOrbRenderer | null
): { element: HTMLElement; orbRenderer: ThinkingOrbRenderer | null } {
  const container = document.createElement('div');
  container.className = `work-group-container ${data.is_expanded ? 'expanded' : ''}`;

  let orbRenderer = existingOrbRenderer || null;

  // ===== 1. HEADER ROW =====
  const header = document.createElement('div');
  header.className = 'work-group-header';

  // Active Thinking Orb (only rendered when model is actively working)
  if (data.is_active) {
    const orbContainer = document.createElement('div');
    orbContainer.className = 'work-group-orb-container';

    const canvas = document.createElement('canvas');
    canvas.className = 'work-group-orb-canvas';
    orbContainer.appendChild(canvas);
    header.appendChild(orbContainer);

    if (!orbRenderer) {
      try {
        orbRenderer = new ThinkingOrbRenderer(canvas);
        orbRenderer.start();
      } catch (e) {
        console.warn('[WorkGroup] Failed to initialize ThinkingOrbRenderer:', e);
      }
    }
  }

  // Summary Text
  const summary = document.createElement('span');
  summary.className = `work-group-summary-text ${data.is_active ? 'active' : ''}`;

  if (data.is_active) {
    summary.textContent = 'Working...';
  } else {
    const baseSummary = getWorkGroupSummaryText(data.items);
    summary.textContent = `${baseSummary} | Worked for ${data.elapsed_secs}s.`;
  }
  header.appendChild(summary);

  // Expand / Collapse Chevron (rotated -90deg when collapsed, 0deg when expanded)
  const chevron = document.createElement('span');
  chevron.className = 'ui-icon icon-tool-chevron work-group-chevron';
  header.appendChild(chevron);

  header.addEventListener('click', () => {
    onToggleExpand();
  });

  container.appendChild(header);

  // ===== 2. EXPANDED TIMELINE LIST =====
  const timeline = document.createElement('div');
  timeline.className = 'work-group-timeline';

  const totalCount = data.items.length;

  data.items.forEach((item, idx) => {
    const row = document.createElement('div');
    row.className = 'work-group-timeline-row';

    // Left Column: 20px timeline spine
    const spine = document.createElement('div');
    spine.className = 'work-group-timeline-spine';

    // 1px Vertical connector line
    const line = document.createElement('div');
    if (totalCount === 1) {
      line.className = 'work-group-timeline-line single';
    } else if (idx === 0) {
      line.className = 'work-group-timeline-line first';
    } else if (idx === totalCount - 1) {
      line.className = 'work-group-timeline-line last';
    } else {
      line.className = 'work-group-timeline-line middle';
    }
    spine.appendChild(line);

    // Dark background mask behind the icon
    const mask = document.createElement('div');
    mask.className = 'work-group-timeline-mask';
    spine.appendChild(mask);

    // 14x14 Checkpoint Icon
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

    // Right Column: Item Content (Thinking text or Tool item)
    const content = document.createElement('div');
    content.className = 'work-group-timeline-content';

    if (item.kind === 'thinking') {
      const textBlock = document.createElement('div');
      textBlock.className = 'work-group-thinking-text';
      textBlock.textContent = item.thinking_text;
      content.appendChild(textBlock);
    } else if (item.kind === 'tool') {
      // Sub-tool Trigger Row
      const toolHeader = document.createElement('div');
      toolHeader.className = 'work-group-tool-header';

      const toolTitle = document.createElement('span');
      toolTitle.className = 'work-group-tool-title';
      toolTitle.textContent = item.tool_title || `Running ${item.tool_name}`;

      const subChevron = document.createElement('span');
      subChevron.className = `ui-icon icon-tool-chevron work-group-tool-chevron ${
        item.is_expanded ? 'expanded' : ''
      }`;

      toolHeader.appendChild(toolTitle);
      toolHeader.appendChild(subChevron);

      toolHeader.addEventListener('click', () => {
        onToggleItem(idx);
      });

      content.appendChild(toolHeader);

      // Collapsible ToolDetailBody: Always shows BOTH args and output
      if (item.is_expanded) {
        const detailBody = document.createElement('div');
        detailBody.className = 'tool-detail-body';

        // 1. Tool Input Arguments Section
        const argsEl = document.createElement('div');
        argsEl.className = 'tool-args-text';
        let formattedArgs = item.tool_args;
        try {
          if (item.tool_args && (item.tool_args.trim().startsWith('{') || item.tool_args.trim().startsWith('['))) {
            const parsed = JSON.parse(item.tool_args);
            formattedArgs = JSON.stringify(parsed, null, 2);
          }
        } catch {
          // Keep raw arguments string if JSON parsing fails
        }
        argsEl.textContent = `args: ${formattedArgs || '{}'}`;
        detailBody.appendChild(argsEl);

        // 2. Horizontal Divider separating Args and Output
        const divider = document.createElement('div');
        divider.className = 'tool-detail-divider';
        detailBody.appendChild(divider);

        // 3. Tool Execution Output Result Section
        const resultEl = document.createElement('div');
        resultEl.className = `tool-result-text ${item.tool_status === 'failed' ? 'failed' : ''}`;
        if (item.tool_result) {
          resultEl.textContent = item.tool_result;
        } else if (item.tool_status === 'running') {
          resultEl.textContent = 'output: Tool is executing...';
        } else {
          resultEl.textContent = 'output: (no output)';
        }
        detailBody.appendChild(resultEl);

        content.appendChild(detailBody);
      }
    }

    row.appendChild(content);
    timeline.appendChild(row);
  });

  container.appendChild(timeline);

  return { element: container, orbRenderer };
}
