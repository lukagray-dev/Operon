// Chat Messages Controller & Ultra-Smooth 60FPS DOM Stream Renderer
//
// Architectural Improvements:
// 1. In-place DOM updates: Incoming text tokens and tool activities mutate only
//    the active message element instead of destroying and recreating the whole conversation DOM.
// 2. Batched RAF updates: Stream text mutations are batched using requestAnimationFrame.
// 3. Single ThinkingOrbRenderer instance: Orb animation loop is cleanly started and destroyed
//    without creating hundreds of leaked canvas animation loops.
// 4. Smart Auto-Scroll: Auto-scrolls only when the user is already near the bottom, allowing
//    smooth, un-interrupted scroll exploration without scroll fighting or jank.

import { refreshSidebarContent } from '../../left-sidebar/sidebar.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { setEmptyStateVisible } from '../empty-state/empty-state.js';
import { inputState } from '../input/state.js';
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
let streamRafId: number | null = null;
let pendingTextUpdate: { element: HTMLElement; text: string } | null = null;

export function initMessages(): void {
  // 1. Listen for session selection changes in the sidebar
  sidebarState.subscribe(async () => {
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

  // 2. Re-render entire message list when full list changes (e.g. session load, clear, message add)
  messagesState.subscribe(() => {
    syncMessageList();
  });

  // 3. Stream text handler: ultra-fast in-place RAF update without destroying DOM
  messagesState.onStreamText((msgId, fullText) => {
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (!row) {
      syncMessageList();
      return;
    }

    let body = row.querySelector<HTMLElement>('.assistant-message-body');
    if (!body) {
      body = document.createElement('div');
      body.className = 'assistant-message-body';
      const actions = row.querySelector('.assistant-action-bar');
      if (actions) {
        row.insertBefore(body, actions);
      } else {
        row.appendChild(body);
      }
    }

    pendingTextUpdate = { element: body, text: fullText };

    if (streamRafId === null) {
      streamRafId = requestAnimationFrame(() => {
        streamRafId = null;
        if (pendingTextUpdate) {
          pendingTextUpdate.element.textContent = pendingTextUpdate.text;
          pendingTextUpdate = null;
        }
        smartAutoScroll();
      });
    }
  });

  // 4. Stream WorkGroup handler: updates timeline in-place
  messagesState.onStreamWorkGroup((msgId) => {
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (!row) return;

    const msg = messagesState.getMessageById(msgId);
    if (!msg || !msg.work_group) return;

    let workGroupContainer = row.querySelector<HTMLElement>('.work-group-container');
    const { element: newEl, orbRenderer } = renderWorkGroupElement(
      msg.work_group,
      () => messagesState.toggleWorkGroupExpanded(msg.id),
      (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, itemIdx),
      activeOrbRenderer
    );

    if (orbRenderer && orbRenderer !== activeOrbRenderer) {
      cleanupActiveOrb();
      activeOrbRenderer = orbRenderer;
    }

    if (workGroupContainer) {
      workGroupContainer.replaceWith(newEl);
    } else {
      row.prepend(newEl);
    }

    smartAutoScroll();
  });

  // 5. Stream finished handler: finalizes workgroup and displays action bar
  messagesState.onStreamFinished((msgId) => {
    cleanupActiveOrb();
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (row) {
      const msg = messagesState.getMessageById(msgId);
      if (msg) {
        // Re-render single row to finalized state
        const updated = createAssistantMessageElement(msg, false);
        row.replaceWith(updated);
      }
    }
    smartAutoScroll();
  });

  // 6. Listen to streaming agent events from Tauri backend
  listenAgentEvent((event) => {
    handleAgentEvent(event);
  });

  // 7. Listen to agent finish turn notification
  listenAgentFinished(async (finishedSessionId) => {
    cleanupActiveOrb();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);

    // Refresh history, sidebar conversation list, and topbar stats
    if (sidebarState.getActiveSessionId() === finishedSessionId) {
      await refreshMessages(finishedSessionId);
    }
    await refreshSidebarContent();
    await refreshTopbar();
  });

  // 8. Listen to agent error notification
  listenAgentError((errMsg) => {
    console.error('[Messages] Agent error received:', errMsg);
    cleanupActiveOrb();
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
  } else if ('Done' in event) {
    cleanupActiveOrb();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);
  } else if ('Error' in event) {
    cleanupActiveOrb();
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
 * Syncs DOM elements with state without destroying the whole message list if not needed.
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

  // Incrementally append new message rows if elements already match
  const existingRows = listContainer.querySelectorAll<HTMLElement>('[data-message-id]');
  if (existingRows.length < messages.length) {
    for (let i = existingRows.length; i < messages.length; i++) {
      const msg = messages[i];
      const isLast = i === messages.length - 1;
      const el = msg.role === 'user' ? createUserMessageElement(msg) : createAssistantMessageElement(msg, !isLast);
      listContainer.appendChild(el);
    }
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

  messages.forEach((msg, idx) => {
    if (msg.role === 'user') {
      listContainer.appendChild(createUserMessageElement(msg));
    } else if (msg.role === 'assistant') {
      const isLast = idx === messages.length - 1;
      listContainer.appendChild(createAssistantMessageElement(msg, !isLast));
    }
  });

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

  // Edit button (loads message back into the input box)
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

function createAssistantMessageElement(msg: ChatMessage, showSeparator: boolean): HTMLElement {
  const row = document.createElement('div');
  row.className = 'assistant-message-row';
  row.setAttribute('data-message-id', msg.id);

  // 1. Render WorkGroup / Tool Activity if present
  if (msg.work_group && msg.work_group.items.length > 0) {
    const { element: workGroupEl, orbRenderer } = renderWorkGroupElement(
      msg.work_group,
      () => messagesState.toggleWorkGroupExpanded(msg.id),
      (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, itemIdx),
      activeOrbRenderer
    );
    if (orbRenderer && orbRenderer !== activeOrbRenderer) {
      cleanupActiveOrb();
      activeOrbRenderer = orbRenderer;
    }
    row.appendChild(workGroupEl);
  }

  // 2. Raw text body
  const body = document.createElement('div');
  body.className = 'assistant-message-body';
  body.textContent = msg.text || (msg.work_group?.is_active ? '' : '...');
  row.appendChild(body);

  // 3. Bottom action bar (shown once response has completed or not currently streaming)
  const isStreaming = messagesState.getStreamingMessageId() === msg.id;
  if (!isStreaming) {
    const bar = document.createElement('div');
    bar.className = 'assistant-action-bar';

    // Left action items
    const barLeft = document.createElement('div');
    barLeft.className = 'assistant-action-bar-left';
    barLeft.innerHTML = `
      <span class="assistant-brand-dot"></span>
      <span class="assistant-time-text">${msg.timestamp}</span>
    `;

    // Right action items
    const barRight = document.createElement('div');
    barRight.className = 'assistant-action-bar-right';

    // Copy button
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
        }, 1500);
      } catch {
        // Fallback
      }
    });

    // Like button
    const likeBtn = document.createElement('button');
    likeBtn.className = `assistant-action-btn ${msg.is_liked ? 'active' : ''}`;
    likeBtn.title = 'Good response';
    likeBtn.innerHTML = '<span class="ui-icon icon-asst-like"></span>';
    likeBtn.addEventListener('click', () => {
      messagesState.toggleLike(msg.id);
    });

    // Dislike button
    const dislikeBtn = document.createElement('button');
    dislikeBtn.className = `assistant-action-btn ${msg.is_disliked ? 'active' : ''}`;
    dislikeBtn.title = 'Bad response';
    dislikeBtn.innerHTML = '<span class="ui-icon icon-asst-dislike"></span>';
    dislikeBtn.addEventListener('click', () => {
      messagesState.toggleDislike(msg.id);
    });

    // Fork button
    const forkBtn = document.createElement('button');
    forkBtn.className = 'assistant-action-btn';
    forkBtn.title = 'Fork from this turn';
    forkBtn.innerHTML = '<span class="ui-icon icon-asst-fork"></span>';
    forkBtn.addEventListener('click', () => {
      console.debug('[Messages] Fork from turn:', msg.turn_index);
    });

    // Redo button
    const redoBtn = document.createElement('button');
    redoBtn.className = 'assistant-action-btn';
    redoBtn.title = 'Regenerate response';
    redoBtn.innerHTML = '<span class="ui-icon icon-asst-redo"></span>';
    redoBtn.addEventListener('click', () => {
      console.debug('[Messages] Regenerate from turn:', msg.turn_index);
    });

    barRight.appendChild(copyBtn);
    barRight.appendChild(likeBtn);
    barRight.appendChild(dislikeBtn);
    barRight.appendChild(forkBtn);
    barRight.appendChild(redoBtn);

    bar.appendChild(barLeft);
    bar.appendChild(barRight);

    row.appendChild(bar);
  }

  if (showSeparator) {
    const sep = document.createElement('div');
    sep.className = 'turn-separator';
    row.appendChild(sep);
  }

  return row;
}
