// Chat Messages Controller & DOM Stream Renderer

import { refreshSidebarContent } from '../../left-sidebar/sidebar.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { setEmptyStateVisible } from '../empty-state/empty-state.js';
import { inputState } from '../input/state.js';
import { refreshTopbar } from '../topbar/topbar.js';
import {
  listenAgentError,
  listenAgentEvent,
  listenAgentFinished,
  loadSessionMessagesIpc,
} from './ipc.js';
import { messagesState } from './state.js';
import type { ChatMessage } from './types.js';

export function initMessages(): void {
  // 1. Listen for session selection changes in the sidebar
  sidebarState.subscribe(async () => {
    const activeSessionId = sidebarState.getActiveSessionId();
    if (activeSessionId) {
      await refreshMessages(activeSessionId);
    } else {
      messagesState.clear();
      setEmptyStateVisible(true);
      renderMessageList();
    }
  });

  // 2. Re-render when message list state updates
  messagesState.subscribe(() => {
    renderMessageList();
  });

  // 3. Listen to streaming agent events from Tauri backend
  listenAgentEvent((event) => {
    handleAgentEvent(event);
  });

  // 4. Listen to agent finish turn notification
  listenAgentFinished(async (finishedSessionId) => {
    messagesState.finishStreaming();
    inputState.setIsResponding(false);

    // Refresh history, sidebar conversation list, and topbar stats
    if (sidebarState.getActiveSessionId() === finishedSessionId) {
      await refreshMessages(finishedSessionId);
    }
    await refreshSidebarContent();
    await refreshTopbar();
  });

  // 5. Listen to agent error notification
  listenAgentError((errMsg) => {
    console.error('[Messages] Agent error received:', errMsg);
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

export async function refreshMessages(sessionId: string): Promise<void> {
  messagesState.setIsLoading(true);
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
  } else if ('Done' in event) {
    messagesState.finishStreaming();
    inputState.setIsResponding(false);
  } else if ('Error' in event) {
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

  // Scroll to bottom
  container.scrollTop = container.scrollHeight;
}

function createUserMessageElement(msg: ChatMessage): HTMLElement {
  const row = document.createElement('div');
  row.className = 'user-message-row';

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

  // Raw text body
  const body = document.createElement('div');
  body.className = 'assistant-message-body';
  body.textContent = msg.text || '...';

  // Bottom action bar
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

  row.appendChild(body);
  row.appendChild(bar);

  if (showSeparator) {
    const sep = document.createElement('div');
    sep.className = 'turn-separator';
    row.appendChild(sep);
  }

  return row;
}
