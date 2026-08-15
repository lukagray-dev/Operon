// Chat Messages Controller & Ultra-Smooth 60FPS DOM Stream Renderer
//
// Features:
// 1. Chronological multi-block rendering (interleaved WorkGroups and Text bodies).
// 2. Chronological thinking checkpoints inside WorkGroup timelines.
// 3. In-place RAF-batched DOM stream updates.
// 4. Stable ThinkingOrbRenderer lifecycle and smart non-intrusive auto-scroll.

import { refreshSidebarContent } from '../../left-sidebar/sidebar.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { setEmptyStateVisible } from '../empty-state/empty-state.js';
import { inputState } from '../input/state.js';
import {
  liveMarkdownRenderer,
  postProcessMarkdownElement,
  renderMarkdownToHtml,
} from '../markdown/markdown.js';
import { hidePermissionDialog, showPermissionDialog } from '../permission/permission.js';
import { refreshTopbar } from '../topbar/topbar.js';
import type { ThinkingOrbRenderer } from '../work-group/orb.js';
import { renderWorkGroupElement } from '../work-group/work-group.js';
import {
  listenAgentError,
  listenAgentEvent,
  listenAgentFinished,
  loadSessionMessagesIpc,
} from './ipc.js';
import { messagesState } from './state.js';
import type { ChatMessage } from './types.js';

let activeOrbRenderer: ThinkingOrbRenderer | null = null;

function insertBlockInRow(row: HTMLElement, newEl: HTMLElement, blockIdx: number): void {
  newEl.setAttribute('data-block-index', String(blockIdx));

  const children = Array.from(row.children) as HTMLElement[];
  let insertBeforeEl: HTMLElement | null = null;

  for (const child of children) {
    const idxAttr = child.getAttribute('data-block-index');
    if (idxAttr !== null) {
      const idx = parseInt(idxAttr, 10);
      if (idx > blockIdx) {
        insertBeforeEl = child;
        break;
      }
    } else if (child.classList.contains('assistant-controls-container') || child.classList.contains('assistant-action-bar') || child.classList.contains('turn-separator')) {
      insertBeforeEl = child;
      break;
    }
  }

  if (insertBeforeEl) {
    row.insertBefore(newEl, insertBeforeEl);
  } else {
    row.appendChild(newEl);
  }
}

export function initMessages(): void {
  // 1. Listen for session selection changes in the sidebar
  sidebarState.subscribe(async () => {
    liveMarkdownRenderer.clearAll();
    const activeSessionId = sidebarState.getActiveSessionId();
    if (activeSessionId) {
      await refreshMessages(activeSessionId);
    } else {
      cleanupActiveOrb();
      messagesState.clear();
      setEmptyStateVisible(true);
      renderMessageList();
    }
  });

  // 2. Full reset handler (when session loads or resets)
  messagesState.onFullReset(() => {
    cleanupActiveOrb();
    liveMarkdownRenderer.clearAll();
    renderMessageList();
  });

  // 3. Incremental sync when items added
  messagesState.subscribe(() => {
    syncMessageList();
  });

  // 4. Stream text handler: ultra-fast live Markdown stream update with RAF batching
  messagesState.onStreamText((msgId, blockIdx, fullBlockText) => {
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (!row) {
      syncMessageList();
      return;
    }

    let body = row.querySelector<HTMLElement>(`[data-block-index="${blockIdx}"].assistant-message-body`);
    if (!body) {
      body = document.createElement('div');
      body.className = 'assistant-message-body markdown-body';
      insertBlockInRow(row, body, blockIdx);
    }

    liveMarkdownRenderer.queueStreamUpdate(body, fullBlockText, () => {
      smartAutoScroll();
    });
  });

  // 5. Stream WorkGroup handler: updates timeline block in-place
  messagesState.onStreamWorkGroup((msgId, blockIdx) => {
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (!row) return;

    const msg = messagesState.getMessageById(msgId);
    if (!msg) return;

    let targetWorkGroup = msg.work_group;
    if (msg.blocks && msg.blocks[blockIdx] && msg.blocks[blockIdx].kind === 'work_group') {
      targetWorkGroup = msg.blocks[blockIdx].data;
    }

    if (!targetWorkGroup) return;

    const existingWgEl = row.querySelector<HTMLElement>(`[data-block-index="${blockIdx}"].work-group-container`);

    const { element: newEl, orbRenderer } = renderWorkGroupElement(
      targetWorkGroup,
      () => messagesState.toggleWorkGroupExpanded(msg.id, blockIdx),
      (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, blockIdx, itemIdx),
      targetWorkGroup.is_active ? activeOrbRenderer : null
    );
    newEl.setAttribute('data-block-index', String(blockIdx));

    if (orbRenderer && orbRenderer !== activeOrbRenderer) {
      cleanupActiveOrb();
      activeOrbRenderer = orbRenderer;
    }

    if (existingWgEl) {
      existingWgEl.replaceWith(newEl);
    } else {
      insertBlockInRow(row, newEl, blockIdx);
    }

    smartAutoScroll();
  });

  // 6. Stream finished handler: finalizes workgroups and displays action bar
  messagesState.onStreamFinished((msgId) => {
    cleanupActiveOrb();
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (row) {
      const msg = messagesState.getMessageById(msgId);
      if (msg) {
        const updated = createAssistantMessageElement(msg, true);
        row.replaceWith(updated);
      }
    }
    smartAutoScroll();
  });

  // 7. Listen to streaming agent events from Tauri backend
  listenAgentEvent((event) => {
    handleAgentEvent(event);
  });

  // 8. Listen to agent finish turn notification
  listenAgentFinished(async (finishedSessionId) => {
    cleanupActiveOrb();
    hidePermissionDialog();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);

    if (sidebarState.getActiveSessionId() === finishedSessionId) {
      await refreshMessages(finishedSessionId);
    }
    await refreshSidebarContent();
    await refreshTopbar();
  });

  // 9. Listen to agent error notification
  listenAgentError((errMsg) => {
    console.error('[Messages] Agent error received:', errMsg);
    cleanupActiveOrb();
    hidePermissionDialog();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);
  });

  // Initial check
  const initialSessionId = sidebarState.getActiveSessionId();
  if (initialSessionId) {
    refreshMessages(initialSessionId);
  } else {
    setEmptyStateVisible(true);
  }
}

function cleanupActiveOrb(): void {
  if (activeOrbRenderer) {
    activeOrbRenderer.destroy();
    activeOrbRenderer = null;
  }
}

export async function refreshMessages(sessionId: string): Promise<void> {
  messagesState.setIsLoading(true);
  cleanupActiveOrb();
  hidePermissionDialog();
  try {
    const list = await loadSessionMessagesIpc(sessionId);
    messagesState.setMessages(list);
    setEmptyStateVisible(list.length === 0);
  } finally {
    messagesState.setIsLoading(false);
  }
}

function handleAgentEvent(event: Record<string, unknown>): void {
  if ('TextDelta' in event) {
    const delta = (event as { TextDelta: { text: string } }).TextDelta;
    if (delta && delta.text) {
      messagesState.appendStreamText(delta.text);
    }
  } else if ('ThinkingDelta' in event) {
    const delta = (event as { ThinkingDelta: { text: string } }).ThinkingDelta;
    if (delta && delta.text) {
      messagesState.appendThinkingDelta(delta.text);
    }
  } else if ('ToolCallStart' in event) {
    const start = (event as { ToolCallStart: { call_id: string; name: string } }).ToolCallStart;
    if (start) {
      messagesState.addToolCallStart(start.call_id, start.name);
    }
  } else if ('ToolCallArgsReady' in event) {
    const args = (event as { ToolCallArgsReady: { call_id: string; name: string; args_json: string } }).ToolCallArgsReady;
    if (args) {
      messagesState.setToolCallArgs(args.call_id, args.args_json);
    }
  } else if ('ToolCallResult' in event) {
    const res = (event as { ToolCallResult: { call_id: string; name: string; is_error: boolean; content_json: string } }).ToolCallResult;
    if (res) {
      messagesState.setToolCallResult(res.call_id, res.content_json, res.is_error);
    }
  } else if ('ApprovalRequired' in event) {
    const req = (event as {
      ApprovalRequired: {
        id: string;
        tool: string;
        path?: string | null;
        reason: string;
        args_json: string;
      };
    }).ApprovalRequired;
    if (req) {
      showPermissionDialog(req.id, req.tool, req.path || null, req.reason, req.args_json);
      smartAutoScroll();
    }
  } else if ('ApprovalGranted' in event) {
    hidePermissionDialog();
  } else if ('PermissionDenied' in event) {
    hidePermissionDialog();
  } else if ('Done' in event) {
    cleanupActiveOrb();
    hidePermissionDialog();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);
  } else if ('Error' in event) {
    cleanupActiveOrb();
    hidePermissionDialog();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);
  } else if ('ContextUsageUpdated' in event) {
    const ctx = (event as { ContextUsageUpdated: { current_context_tokens: number; context_window: number; utilization: number } }).ContextUsageUpdated;
    if (ctx) {
      const formatted = ctx.current_context_tokens >= 1000
        ? `${(ctx.current_context_tokens / 1000).toFixed(1)}k / ${Math.round(ctx.context_window / 1000)}k`
        : `${ctx.current_context_tokens} / ${Math.round(ctx.context_window / 1000)}k`;

      inputState.setContextUsage({
        tokens_used: ctx.current_context_tokens,
        tokens_total: ctx.context_window,
        percentage: ctx.utilization * 100,
        formatted,
      });
    }
  }
}

/**
 * Syncs DOM elements incrementally when items are appended.
 */
function syncMessageList(): void {
  const container = document.getElementById('chat-messages-viewport');
  if (!container) return;

  const messages = messagesState.getMessages();
  if (messages.length === 0) {
    setEmptyStateVisible(true);
    const oldList = document.getElementById('chat-messages-list');
    if (oldList) oldList.remove();
    return;
  }

  setEmptyStateVisible(false);

  let listContainer = document.getElementById('chat-messages-list');
  if (!listContainer) {
    renderMessageList();
    return;
  }

  const existingRows = listContainer.querySelectorAll<HTMLElement>('[data-message-id]');
  if (existingRows.length < messages.length) {
    const oldSpacer = listContainer.querySelector('.chat-bottom-spacer');
    if (oldSpacer) oldSpacer.remove();

    for (let i = existingRows.length; i < messages.length; i++) {
      const msg = messages[i];
      const el = msg.role === 'user' ? createUserMessageElement(msg) : createAssistantMessageElement(msg, true);
      listContainer.appendChild(el);
    }

    const spacer = document.createElement('div');
    spacer.className = 'chat-bottom-spacer';
    listContainer.appendChild(spacer);

    smartAutoScroll();
  } else if (existingRows.length > messages.length) {
    renderMessageList();
  }
}

function renderMessageList(): void {
  const container = document.getElementById('chat-messages-viewport');
  if (!container) return;

  const emptyStateEl = document.getElementById('chat-empty-state');

  // Clear existing messages container
  const oldList = document.getElementById('chat-messages-list');
  if (oldList) {
    oldList.remove();
  }

  const messages = messagesState.getMessages();
  if (messages.length === 0) {
    setEmptyStateVisible(true);
    return;
  }

  setEmptyStateVisible(false);

  const listContainer = document.createElement('div');
  listContainer.id = 'chat-messages-list';
  listContainer.className = 'messages-container';

  messages.forEach((msg) => {
    if (msg.role === 'user') {
      listContainer.appendChild(createUserMessageElement(msg));
    } else if (msg.role === 'assistant') {
      listContainer.appendChild(createAssistantMessageElement(msg, true));
    }
  });

  const spacer = document.createElement('div');
  spacer.className = 'chat-bottom-spacer';
  listContainer.appendChild(spacer);

  if (emptyStateEl) {
    container.insertBefore(listContainer, emptyStateEl);
  } else {
    container.appendChild(listContainer);
  }

  smartAutoScroll(true);
}

function smartAutoScroll(force = false): void {
  const container = document.getElementById('chat-messages-viewport');
  if (!container) return;

  const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight <= 120;
  if (force || isNearBottom) {
    container.scrollTop = container.scrollHeight;
  }
}

function createUserMessageElement(msg: ChatMessage): HTMLElement {
  const row = document.createElement('div');
  row.className = 'user-message-row';
  row.setAttribute('data-message-id', msg.id);

  const bubble = document.createElement('div');
  bubble.className = 'user-message-bubble';
  bubble.textContent = msg.text;

  const actions = document.createElement('div');
  actions.className = 'user-message-actions';

  // Copy button
  const copyBtn = document.createElement('button');
  copyBtn.className = 'user-action-btn';
  copyBtn.title = 'Copy message';
  copyBtn.innerHTML = '<span class="ui-icon icon-msg-copy"></span>';

  copyBtn.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(msg.text);
      copyBtn.innerHTML = '<span class="ui-icon icon-msg-check"></span>';
      setTimeout(() => {
        copyBtn.innerHTML = '<span class="ui-icon icon-msg-copy"></span>';
      }, 1500);
    } catch {
      // Fallback
    }
  });

  // Edit button
  const editBtn = document.createElement('button');
  editBtn.className = 'user-action-btn';
  editBtn.title = 'Edit prompt';
  editBtn.innerHTML = '<span class="ui-icon icon-msg-edit"></span>';

  editBtn.addEventListener('click', () => {
    const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
    if (textarea) {
      textarea.value = msg.text;
      textarea.style.height = 'auto';
      const newHeight = Math.min(200, Math.max(42, textarea.scrollHeight));
      textarea.style.height = `${newHeight}px`;
      textarea.focus();
      inputState.setInputText(msg.text);
    }
  });

  actions.appendChild(copyBtn);
  actions.appendChild(editBtn);

  row.appendChild(bubble);
  row.appendChild(actions);

  return row;
}

function renderMarkdownBody(element: HTMLElement, markdownText: string): void {
  element.className = 'assistant-message-body markdown-body';
  if (!markdownText || markdownText.trim().length === 0) {
    element.textContent = '...';
    return;
  }

  renderMarkdownToHtml(markdownText)
    .then((html) => {
      element.innerHTML = html;
      postProcessMarkdownElement(element);
    })
    .catch((err) => {
      console.error('[Messages] Markdown render error:', err);
      element.textContent = markdownText;
    });
}

function createAssistantMessageElement(msg: ChatMessage, showSeparator = true): HTMLElement {
  const row = document.createElement('div');
  row.className = 'assistant-message-row';
  row.setAttribute('data-message-id', msg.id);

  // Render chronological multi-block list if present
  if (msg.blocks && msg.blocks.length > 0) {
    msg.blocks.forEach((block, blockIdx) => {
      if (block.kind === 'work_group') {
        if (block.data.items.length > 0 || block.data.is_active) {
          const { element: workGroupEl, orbRenderer } = renderWorkGroupElement(
            block.data,
            () => messagesState.toggleWorkGroupExpanded(msg.id, blockIdx),
            (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, blockIdx, itemIdx),
            block.data.is_active ? activeOrbRenderer : null
          );
          workGroupEl.setAttribute('data-block-index', String(blockIdx));
          if (orbRenderer && orbRenderer !== activeOrbRenderer) {
            cleanupActiveOrb();
            activeOrbRenderer = orbRenderer;
          }
          row.appendChild(workGroupEl);
        }
      } else if (block.kind === 'text') {
        if (block.text.length > 0) {
          const body = document.createElement('div');
          body.className = 'assistant-message-body markdown-body';
          body.setAttribute('data-block-index', String(blockIdx));
          renderMarkdownBody(body, block.text);
          row.appendChild(body);
        }
      }
    });
  } else {
    // Fallback single WorkGroup and single text body
    if (msg.work_group && (msg.work_group.items.length > 0 || msg.work_group.is_active)) {
      const { element: workGroupEl, orbRenderer } = renderWorkGroupElement(
        msg.work_group,
        () => messagesState.toggleWorkGroupExpanded(msg.id, 0),
        (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, 0, itemIdx),
        activeOrbRenderer
      );
      if (orbRenderer && orbRenderer !== activeOrbRenderer) {
        cleanupActiveOrb();
        activeOrbRenderer = orbRenderer;
      }
      row.appendChild(workGroupEl);
    }

    if (msg.text) {
      const body = document.createElement('div');
      body.className = 'assistant-message-body markdown-body';
      renderMarkdownBody(body, msg.text);
      row.appendChild(body);
    }
  }

  // If nothing rendered yet, add placeholder
  if (!row.querySelector('.assistant-message-body') && !row.querySelector('.work-group-container')) {
    const body = document.createElement('div');
    body.className = 'assistant-message-body markdown-body';
    body.textContent = '...';
    row.appendChild(body);
  }

  // Bottom controls container with separator line and action bar (shown once turn is finalized)
  const isStreaming = messagesState.getStreamingMessageId() === msg.id;
  if (!isStreaming) {
    const controlsContainer = document.createElement('div');
    controlsContainer.className = `assistant-controls-container ${msg.is_liked || msg.is_disliked ? 'has-active' : ''}`;

    // 1. Separator Line
    if (showSeparator) {
      const separatorLine = document.createElement('div');
      separatorLine.className = 'assistant-separator-line';
      controlsContainer.appendChild(separatorLine);
    }

    // 2. Action Bar matching Slint (Operon Logo -> Copy -> Like -> Dislike -> Fork -> Time)
    const bar = document.createElement('div');
    bar.className = 'assistant-action-bar';

    // 2.1 Operon Brand Logo (20px)
    const logoImg = document.createElement('img');
    logoImg.className = 'assistant-brand-logo';
    logoImg.src = 'assets/brand/operon.svg';
    logoImg.alt = 'Operon';
    bar.appendChild(logoImg);

    // 2.2 Copy button
    const copyBtn = document.createElement('button');
    copyBtn.className = 'assistant-action-btn';
    copyBtn.title = 'Copy response';
    copyBtn.innerHTML = '<span class="ui-icon icon-asst-copy"></span>';

    copyBtn.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(msg.text);
        copyBtn.innerHTML = '<span class="ui-icon icon-asst-check"></span>';
        setTimeout(() => {
          copyBtn.innerHTML = '<span class="ui-icon icon-asst-copy"></span>';
        }, 3000);
      } catch {
        // Fallback
      }
    });
    bar.appendChild(copyBtn);

    // 2.3 Like button
    const likeBtn = document.createElement('button');
    likeBtn.className = `assistant-action-btn ${msg.is_liked ? 'active' : ''}`;
    likeBtn.title = 'Good response';
    likeBtn.innerHTML = '<span class="ui-icon icon-asst-like"></span>';
    likeBtn.addEventListener('click', () => {
      messagesState.toggleLike(msg.id);
    });
    bar.appendChild(likeBtn);

    // 2.4 Dislike button
    const dislikeBtn = document.createElement('button');
    dislikeBtn.className = `assistant-action-btn ${msg.is_disliked ? 'active' : ''}`;
    dislikeBtn.title = 'Bad response';
    dislikeBtn.innerHTML = '<span class="ui-icon icon-asst-dislike"></span>';
    dislikeBtn.addEventListener('click', () => {
      messagesState.toggleDislike(msg.id);
    });
    bar.appendChild(dislikeBtn);

    // 2.5 Fork button
    const forkBtn = document.createElement('button');
    forkBtn.className = 'assistant-action-btn';
    forkBtn.title = 'Fork from this turn';
    forkBtn.innerHTML = '<span class="ui-icon icon-asst-fork"></span>';
    forkBtn.addEventListener('click', () => {
      console.debug('[Messages] Fork from turn:', msg.turn_index);
    });
    bar.appendChild(forkBtn);

    // 2.6 Time display
    const timeEl = document.createElement('span');
    timeEl.className = 'assistant-time-text';
    timeEl.textContent = msg.timestamp;
    bar.appendChild(timeEl);

    controlsContainer.appendChild(bar);
    row.appendChild(controlsContainer);
  }

  return row;
}
