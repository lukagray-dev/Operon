// Chat Messages Controller & Ultra-Smooth 60FPS DOM Stream Renderer
//
// Features:
// 1. Chronological multi-block rendering (interleaved WorkGroups and Text bodies).
// 2. Chronological thinking checkpoints inside WorkGroup timelines.
// 3. In-place RAF-batched DOM stream updates.
// 4. Stable ThinkingOrbRenderer lifecycle and smart non-intrusive auto-scroll.

import { forkSessionIpc } from '../../left-sidebar/ipc.js';
import { refreshSidebarContent } from '../../left-sidebar/sidebar.js';
import { sidebarState } from '../../left-sidebar/state.js';
import { getGeneralSettingsIpc } from '../../settings/general/ipc.js';
import type { GeneralSettings } from '../../settings/general/types.js';
import { showConfirmDialog } from '../../shared/dialog.js';
import { listenIpcEvent } from '../../shared/ipc.js';
import { setEmptyStateVisible } from '../empty-state/empty-state.js';
import { getContextUsageIpc } from '../input/ipc.js';
import { inputState } from '../input/state.js';
import {
  liveMarkdownRenderer,
  postProcessMarkdownElement,
  renderMarkdownToHtml,
} from '../markdown/markdown.js';
import {
  hidePermissionDialog,
  showPermissionDialog,
  syncPendingPermissionForActiveSession,
} from '../permission/permission.js';
import { approvePermissionIpc } from './ipc.js';
import { createAskCardElement, resolveAskCardElement } from './ask-card/ask-card.js';
import { respondToAskIpc } from './ask-card/ipc.js';
import { refreshTopbar } from '../topbar/topbar.js';
import type { ThinkingOrbRenderer } from '../work-group/orb.js';
import { renderWorkGroupElement, syncWorkGroupElement } from '../work-group/work-group.js';
import { renderCompactionElement, syncCompactionElement } from './compaction-pill.js';
import {
  editAndSubmitPromptIpc,
  listenAgentError,
  listenAgentEvent,
  listenAgentFinished,
  loadSessionMessagesIpc,
  sendDesktopNotificationIpc,
} from './ipc.js';
import { messagesState } from './state.js';
import type { ChatMessage } from './types.js';

let activeOrbRenderer: ThinkingOrbRenderer | null = null;
let userIsScrolledUp = false;
let cachedGeneralSettings: GeneralSettings | null = null;

async function getCachedSettings(): Promise<GeneralSettings | null> {
  if (!cachedGeneralSettings) {
    try {
      cachedGeneralSettings = await getGeneralSettingsIpc();
    } catch {
      // Ignored
    }
  }
  return cachedGeneralSettings;
}

// Invalidate and sync settings cache whenever user updates General settings
listenIpcEvent<GeneralSettings>('operon://general-settings-changed', (settings) => {
  cachedGeneralSettings = settings;
});

function setupScrollListener(): void {
  const container = document.getElementById('chat-messages-viewport');
  if (!container) return;

  container.addEventListener(
    'scroll',
    () => {
      const distanceFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
      // If user scrolled up by more than 30px, pause autoscroll
      if (distanceFromBottom > 30) {
        userIsScrolledUp = true;
      } else {
        userIsScrolledUp = false;
      }
    },
    { passive: true }
  );

  // If user interacts with an expanded tool detail body, pause autoscroll
  container.addEventListener(
    'mouseenter',
    (e) => {
      const target = e.target as HTMLElement | null;
      if (target && target.closest('.tool-detail-body')) {
        userIsScrolledUp = true;
      }
    },
    true
  );
}

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

function isChannelSession(sessionId: string | null): boolean {
  if (!sessionId) return false;
  if (sessionId.startsWith('wa-') || sessionId.startsWith('tg-')) return true;
  const isWa = sidebarState
    .getWhatsAppContacts()
    .some((c) => c.conversations.some((s) => s.id === sessionId));
  const isTg = sidebarState
    .getTelegramContacts()
    .some((c) => c.conversations.some((s) => s.id === sessionId));
  return isWa || isTg;
}

export function initMessages(): void {
  // Attach scroll tracking to protect user scrolling position
  setupScrollListener();
  let previousActiveSessionId: string | null = sidebarState.getActiveSessionId();

  // 1. Listen for session selection changes in the sidebar
  sidebarState.subscribe(async () => {
    const activeSessionId = sidebarState.getActiveSessionId();
    const isChannel = isChannelSession(activeSessionId);
    inputState.setReadOnly(isChannel, isChannel ? 'Channel conversation (read-only in GUI)' : '');

    if (activeSessionId === previousActiveSessionId) {
      return; // Active session didn't change: preserve existing live DOM and scroll position
    }
    previousActiveSessionId = activeSessionId;

    // If currently streaming a response for this session, protect in-memory streaming messages
    if (inputState.getIsResponding()) {
      syncPendingPermissionForActiveSession(activeSessionId);
      return;
    }

    liveMarkdownRenderer.clearAll();
    syncPendingPermissionForActiveSession(activeSessionId);
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

    const { element: wgEl, orbRenderer } = syncWorkGroupElement(
      existingWgEl,
      targetWorkGroup,
      () => messagesState.toggleWorkGroupExpanded(msg.id, blockIdx),
      (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, blockIdx, itemIdx),
      targetWorkGroup.is_active ? activeOrbRenderer : null
    );
    wgEl.setAttribute('data-block-index', String(blockIdx));

    if (orbRenderer && orbRenderer !== activeOrbRenderer) {
      cleanupActiveOrb();
      activeOrbRenderer = orbRenderer;
    }

    if (!existingWgEl) {
      insertBlockInRow(row, wgEl, blockIdx);
    }

    smartAutoScroll();
  });

  // 6. Stream Ask Question handler: renders interactive clarification prompt inline in the message row
  messagesState.onStreamAskQuestion((msgId, blockIdx, data) => {
    cleanupActiveOrb();
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (row) {
      const askCardEl = createAskCardElement(data, async (answer) => {
        await respondToAskIpc(data.id, answer);
        messagesState.resolveAskQuestion(data.id, answer);
      });
      insertBlockInRow(row, askCardEl, blockIdx);
      smartAutoScroll(true);
    }
  });

  // 6b. Stream Compaction handler: renders expandable compaction pill in message row
  messagesState.onStreamCompaction((msgId, blockIdx, data) => {
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (row) {
      const existingEl = row.querySelector<HTMLElement>(`[data-block-index="${blockIdx}"].compaction-pill-container`);
      if (existingEl) {
        syncCompactionElement(existingEl, data);
      } else {
        const compactionEl = renderCompactionElement(data, () =>
          messagesState.toggleCompactionExpanded(msgId, blockIdx)
        );
        compactionEl.setAttribute('data-block-index', String(blockIdx));
        insertBlockInRow(row, compactionEl, blockIdx);
      }
      smartAutoScroll();
    }
  });

  // 7. Stream finished handler: finalizes workgroups and displays action bar in-place
  messagesState.onStreamFinished((msgId) => {
    cleanupActiveOrb();
    const row = document.querySelector<HTMLElement>(`[data-message-id="${msgId}"]`);
    if (row) {
      const msg = messagesState.getMessageById(msgId);
      if (msg) {
        finalizeAssistantMessageRow(row, msg, true);
      }
    }
  });

  // 7. Listen to streaming agent events from Tauri backend
  listenAgentEvent((event) => {
    handleAgentEvent(event);
  });

  // 8. Listen to agent finish turn notification (turn complete without wiping DOM)
  listenAgentFinished(async (sessionId) => {
    cleanupActiveOrb();
    hidePermissionDialog();
    messagesState.finishStreaming();
    inputState.setIsResponding(false);

    triggerTurnCompleteNotification();

    const activeSession = sidebarState.getActiveSessionId() || sessionId;
    if (activeSession) {
      try {
        const usage = await getContextUsageIpc(activeSession);
        inputState.setContextUsage(usage);
      } catch (err) {
        console.debug('[Messages] Failed to update context usage on finish:', err);
      }
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

  // 10. Listen to notify filesystem watcher for real-time external channel session updates (WhatsApp/Telegram)
  listenIpcEvent<string[]>('sessions-changed', async (changedIds) => {
    const activeSessionId = sidebarState.getActiveSessionId();
    if (!activeSessionId) return;

    // Only channel sessions are hot-reloaded from notify watcher!
    // General chats and project sessions are managed directly by GUI in-memory state.
    if (!isChannelSession(activeSessionId)) {
      return;
    }

    // CRITICAL: NEVER reload while prompt execution is generating in the GUI to prevent flickering!
    const isResponding = inputState.getIsResponding() || messagesState.getStreamingMessageId() !== null;
    if (isResponding) {
      return;
    }

    const ids = changedIds || [];
    if (ids.length === 0 || ids.includes(activeSessionId)) {
      await refreshMessages(activeSessionId);
    }
  });

  // Initial check
  const initialSessionId = sidebarState.getActiveSessionId();
  if (initialSessionId) {
    const isChannel = isChannelSession(initialSessionId);
    inputState.setReadOnly(isChannel, isChannel ? 'Channel conversation (read-only in GUI)' : '');
    refreshMessages(initialSessionId);
  } else {
    setEmptyStateVisible(true);
  }
}

async function triggerPermissionNotification(tool: string, path?: string | null): Promise<void> {
  try {
    const settings = await getCachedSettings();
    if (!settings || !settings.notify_on_permission_request) return;

    await sendDesktopNotificationIpc(
      'Operon — Permission Required',
      `Operon requests permission to ${tool}${path ? ` on ${path}` : ''}`
    );
  } catch (err) {
    console.debug('[Notification] Permission notification error:', err);
  }
}

async function triggerTurnCompleteNotification(): Promise<void> {
  try {
    const settings = await getCachedSettings();
    if (!settings || !settings.notify_on_response_complete) return;

    await sendDesktopNotificationIpc(
      'Operon — Response Complete',
      'The assistant has finished responding.'
    );
  } catch (err) {
    console.debug('[Notification] Turn complete notification error:', err);
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
    try {
      const usage = await getContextUsageIpc(sessionId);
      inputState.setContextUsage(usage, true);
    } catch (err) {
      console.debug('[Messages] Failed to fetch session context usage:', err);
    }
  } finally {
    messagesState.setIsLoading(false);
    syncPendingPermissionForActiveSession(sidebarState.getActiveSessionId());
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
      const activeSess = sidebarState.getActiveSessionId();
      if (inputState.isAutoApproveEnabled()) {
        // Auto-approve mode active: immediately approve without prompting
        approvePermissionIpc(req.id).catch((err: unknown) => {
          console.error('[Messages] Auto-approve permission failed:', err);
        });
      } else {
        showPermissionDialog(req.id, req.tool, req.path || null, req.reason, req.args_json, activeSess);
        triggerPermissionNotification(req.tool, req.path);
        smartAutoScroll();
      }
    }
  } else if ('AskQuestion' in event) {
    const ask = (event as {
      AskQuestion: {
        id: string;
        question: string;
        options: string[];
      };
    }).AskQuestion;
    if (ask) {
      messagesState.addAskQuestion(ask.id, ask.question, ask.options);
      smartAutoScroll(true);
    }
  } else if ('ApprovalGranted' in event || 'PermissionDenied' in event) {
    syncPendingPermissionForActiveSession(sidebarState.getActiveSessionId());
  } else if ('CompactionOccurred' in event) {
    const comp = (event as {
      CompactionOccurred: {
        tokens_before: number;
        tokens_after: number;
        summary: string;
      };
    }).CompactionOccurred;
    if (comp) {
      messagesState.addCompactionBlock(comp.tokens_before, comp.tokens_after, comp.summary);
      smartAutoScroll(true);
    }
  } else if ('Done' in event || 'Error' in event) {
    cleanupActiveOrb();
    syncPendingPermissionForActiveSession(sidebarState.getActiveSessionId());
    messagesState.finishStreaming();
    inputState.setIsResponding(false);
  } else if ('ContextUsageUpdated' in event) {
    const ctx = (event as {
      ContextUsageUpdated: {
        current_context_tokens: number;
        context_window: number;
        utilization: number;
      };
    }).ContextUsageUpdated;
    if (ctx) {
      const total = ctx.context_window;
      const totalStr =
        total >= 1_000_000
          ? `${Number.isInteger(total / 1_000_000) ? total / 1_000_000 : (total / 1_000_000).toFixed(1)}M`
          : `${Math.round(total / 1000)}k`;
      const used = ctx.current_context_tokens;
      const formatted =
        used >= 1_000_000
          ? `${(used / 1_000_000).toFixed(1)}M / ${totalStr}`
          : used >= 1000
          ? `${(used / 1000).toFixed(1)}k / ${totalStr}`
          : `${used} / ${totalStr}`;

      inputState.setContextUsage({
        tokens_used: used,
        tokens_total: total,
        percentage: ctx.utilization * 100,
        formatted,
      });
    }
  } else if ('TokenUsageUpdated' in event) {
    const usage = (event as {
      TokenUsageUpdated: {
        input_tokens: number;
        output_tokens: number;
        context_total: number;
      };
    }).TokenUsageUpdated;
    if (usage) {
      const current = inputState.getContextUsage();
      const total = current.tokens_total > 0 ? current.tokens_total : 128000;
      const totalStr =
        total >= 1_000_000
          ? `${Number.isInteger(total / 1_000_000) ? total / 1_000_000 : (total / 1_000_000).toFixed(1)}M`
          : `${Math.round(total / 1000)}k`;
      const used = usage.context_total;
      const formatted =
        used >= 1_000_000
          ? `${(used / 1_000_000).toFixed(1)}M / ${totalStr}`
          : used >= 1000
          ? `${(used / 1000).toFixed(1)}k / ${totalStr}`
          : `${used} / ${totalStr}`;

      inputState.setContextUsage({
        tokens_used: used,
        tokens_total: total,
        percentage: (used / total) * 100,
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

export function smartAutoScroll(force = false): void {
  const container = document.getElementById('chat-messages-viewport');
  if (!container) return;

  if (force) {
    userIsScrolledUp = false;
    container.scrollTop = container.scrollHeight;
    return;
  }

  // If user scrolled up or auto_scroll_stream is disabled in settings, do not force scroll
  if (userIsScrolledUp) {
    return;
  }

  // If user is hovering over any tool detail body, do not force scroll
  const hoveredToolBody = container.querySelector('.tool-detail-body:hover');
  if (hoveredToolBody) {
    return;
  }

  if (cachedGeneralSettings && cachedGeneralSettings.auto_scroll_stream === false) {
    return;
  }

  const isNearBottom = container.scrollHeight - container.scrollTop - container.clientHeight <= 60;
  if (isNearBottom) {
    container.scrollTop = container.scrollHeight;
  }
}

function createUserMessageElement(msg: ChatMessage): HTMLElement {
  const row = document.createElement('div');
  row.className = 'user-message-row';
  row.setAttribute('data-message-id', msg.id);

  const bubble = document.createElement('div');
  bubble.className = 'user-message-bubble markdown-body';
  bubble.textContent = msg.text;

  // Compile Markdown asynchronously and enhance with KaTeX, highlight.js, and code cards
  renderMarkdownToHtml(msg.text)
    .then((html) => {
      bubble.innerHTML = html;
      postProcessMarkdownElement(bubble);
    })
    .catch((err) => {
      console.error('[User Message] Markdown compilation error:', err);
      bubble.textContent = msg.text;
    });

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
    // If already editing this message, do not create duplicate
    if (row.querySelector('.user-edit-container')) {
      return;
    }

    // Cancel any other message that might currently be in edit mode
    document.querySelectorAll<HTMLElement>('.user-edit-container').forEach((el) => {
      const parentRow = el.closest('.user-message-row');
      if (parentRow) {
        const origBubble = parentRow.querySelector<HTMLElement>('.user-message-bubble');
        const origActions = parentRow.querySelector<HTMLElement>('.user-message-actions');
        if (origBubble) origBubble.style.display = '';
        if (origActions) origActions.style.display = '';
        el.remove();
      }
    });

    // Hide original bubble and action buttons
    bubble.style.display = 'none';
    actions.style.display = 'none';

    // Create inline edit container
    const editContainer = document.createElement('div');
    editContainer.className = 'user-edit-container';

    const textarea = document.createElement('textarea');
    textarea.className = 'user-edit-textarea';
    textarea.value = msg.text;
    textarea.rows = 1;
    textarea.placeholder = 'Edit prompt...';

    // Auto-adjust height based on content
    const autoResize = () => {
      textarea.style.height = 'auto';
      textarea.style.height = `${Math.min(300, Math.max(38, textarea.scrollHeight))}px`;
    };

    textarea.addEventListener('input', autoResize);

    const editActions = document.createElement('div');
    editActions.className = 'user-edit-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'user-edit-btn btn-user-edit-cancel';
    cancelBtn.textContent = 'Cancel';

    const saveBtn = document.createElement('button');
    saveBtn.className = 'user-edit-btn btn-user-edit-save';
    saveBtn.textContent = 'Save';

    const cancelEdit = () => {
      editContainer.remove();
      bubble.style.display = '';
      actions.style.display = '';
    };

    const submitEdit = async () => {
      const newText = textarea.value.trim();
      if (!newText) return;

      const activeSessionId = sidebarState.getActiveSessionId();
      if (!activeSessionId) return;

      const workspacePath = sidebarState.getActiveProjectPath();

      // Slices UI messages: removes all turns from msg.turn_index onwards and inserts updated prompt
      messagesState.truncateAndStartTurn(msg.turn_index, newText);
      inputState.setIsResponding(true);

      // Invoke backend edit and submit
      try {
        await editAndSubmitPromptIpc(activeSessionId, newText, msg.turn_index, workspacePath);
      } catch (err) {
        console.error('[Messages] Edit submit error:', err);
        inputState.setIsResponding(false);
      }
    };

    cancelBtn.addEventListener('click', cancelEdit);
    saveBtn.addEventListener('click', submitEdit);

    textarea.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cancelEdit();
      } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        submitEdit();
      }
    });

    editActions.appendChild(cancelBtn);
    editActions.appendChild(saveBtn);

    editContainer.appendChild(textarea);
    editContainer.appendChild(editActions);

    row.insertBefore(editContainer, actions);

    // Initial resize and focus
    requestAnimationFrame(() => {
      autoResize();
      textarea.focus();
      textarea.setSelectionRange(textarea.value.length, textarea.value.length);
    });
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
      } else if (block.kind === 'compaction') {
        const compactionEl = renderCompactionElement(block.data, () =>
          messagesState.toggleCompactionExpanded(msg.id, blockIdx)
        );
        compactionEl.setAttribute('data-block-index', String(blockIdx));
        row.appendChild(compactionEl);
      } else if (block.kind === 'ask') {
        const askCardEl = createAskCardElement(block.data, async (answer) => {
          await respondToAskIpc(block.data.id, answer);
          messagesState.resolveAskQuestion(block.data.id, answer);
        });
        askCardEl.setAttribute('data-block-index', String(blockIdx));
        row.appendChild(askCardEl);
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
    const controlsContainer = createAssistantActionBar(msg, showSeparator);
    row.appendChild(controlsContainer);
  }

  return row;
}

function createAssistantActionBar(msg: ChatMessage, showSeparator = true): HTMLElement {
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
  forkBtn.addEventListener('click', async () => {
    const currentSessionId = sidebarState.getActiveSessionId();
    if (!currentSessionId) return;

    const confirmed = await showConfirmDialog({
      title: 'Fork Conversation',
      message: 'Fork this conversation from this turn into a new chat?',
      confirmText: 'Ok',
      cancelText: 'Cancel',
      icon: 'help',
    });
    if (!confirmed) return;

    try {
      const newSessionId = await forkSessionIpc(currentSessionId, msg.turn_index);
      if (newSessionId) {
        const projectPath = sidebarState.getActiveProjectPath();
        sidebarState.selectSession(newSessionId, projectPath);
        await refreshSidebarContent();
        await refreshMessages(newSessionId);
        await refreshTopbar();
      }
    } catch (err) {
      console.error('[Messages] Fork from turn error:', err);
    }
  });
  bar.appendChild(forkBtn);

  // 2.6 Time display
  const timeEl = document.createElement('span');
  timeEl.className = 'assistant-time-text';
  timeEl.textContent = msg.timestamp;
  bar.appendChild(timeEl);

  controlsContainer.appendChild(bar);
  return controlsContainer;
}

function finalizeAssistantMessageRow(row: HTMLElement, msg: ChatMessage, showSeparator = true): void {
  // 1. Ensure all WorkGroups inside row are finalized in-place
  if (msg.blocks && msg.blocks.length > 0) {
    msg.blocks.forEach((block, blockIdx) => {
      if (block.kind === 'work_group') {
        const existingWgEl = row.querySelector<HTMLElement>(`[data-block-index="${blockIdx}"].work-group-container`);
        if (existingWgEl) {
          syncWorkGroupElement(
            existingWgEl,
            block.data,
            () => messagesState.toggleWorkGroupExpanded(msg.id, blockIdx),
            (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, blockIdx, itemIdx),
            null
          );
        }
      } else if (block.kind === 'text') {
        const body = row.querySelector<HTMLElement>(`[data-block-index="${blockIdx}"].assistant-message-body`);
        if (body) {
          liveMarkdownRenderer.finalizeStream(body, block.text);
        }
      } else if (block.kind === 'ask') {
        const existingAskEl = row.querySelector<HTMLElement>(`[data-block-index="${blockIdx}"].ask-card`);
        if (existingAskEl && block.data.is_answered && block.data.answer) {
          resolveAskCardElement(existingAskEl, block.data.answer);
        }
      }
    });
  } else {
    if (msg.work_group) {
      const existingWgEl = row.querySelector<HTMLElement>('.work-group-container');
      if (existingWgEl) {
        syncWorkGroupElement(
          existingWgEl,
          msg.work_group,
          () => messagesState.toggleWorkGroupExpanded(msg.id, 0),
          (itemIdx) => messagesState.toggleWorkGroupItemExpanded(msg.id, 0, itemIdx),
          null
        );
      }
    }
    if (msg.text) {
      const body = row.querySelector<HTMLElement>('.assistant-message-body');
      if (body) {
        liveMarkdownRenderer.finalizeStream(body, msg.text);
      }
    }
  }

  // 2. Add action bar if not already present
  const existingControls = row.querySelector('.assistant-controls-container');
  if (!existingControls) {
    const controlsContainer = createAssistantActionBar(msg, showSeparator);
    row.appendChild(controlsContainer);
  }
}
