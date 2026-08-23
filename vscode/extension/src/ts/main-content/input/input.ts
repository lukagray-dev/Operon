// Main Content Input Panel Controller for VS Code

import { sidebarState } from '../../left-sidebar/state.js';
import { cancelPromptIpc, submitPromptIpc } from '../messages/ipc.js';
import { messagesState } from '../messages/state.js';
import type { ChatMessage } from '../messages/types.js';
import { getGeneralSettingsIpc } from '../../settings/general/ipc.js';
import { listenIpcEvent } from '../../shared/ipc.js';
import {
  getAvailableModelsIpc,
  getContextUsageIpc,
  pickAttachmentsIpc,
  selectModelIpc,
  toggleAutoApproveIpc,
} from './ipc.js';
import { inputState } from './state.js';
import type { ModelOption } from './types.js';
import { stopVoiceRecording, toggleVoiceRecording } from './voice.js';

let activePopover: HTMLElement | null = null;
let activeSubDropdown: HTMLElement | null = null;
let subDropdownHoverTimeout: number | null = null;

export function initInputPanel(): void {
  setupTextarea();
  setupAttachButton();
  setupAutoApproveButton();
  setupModelSelector();
  setupVoiceButton();
  setupSendButton();
  setupOutsideClickListener();

  // Re-render when input state changes
  inputState.subscribe(() => {
    renderInputState();
  });

  // Render initial static defaults
  renderInputState();

  // Initial async load
  loadInitialInputData();

  // Listen to broadcast events from settings window
  listenIpcEvent<boolean>('operon://auto-approve-changed', (enabled) => {
    inputState.setAutoApproveEnabled(enabled);
    renderInputState();
  });

  // Also re-verify on window focus
  window.addEventListener('focus', () => {
    loadInitialInputData();
  });
}

export function dismissSubDropdown(): void {
  if (subDropdownHoverTimeout !== null) {
    window.clearTimeout(subDropdownHoverTimeout);
    subDropdownHoverTimeout = null;
  }
  if (activeSubDropdown) {
    activeSubDropdown.remove();
    activeSubDropdown = null;
  }
}

export function dismissPopover(): void {
  dismissSubDropdown();
  if (activePopover) {
    activePopover.remove();
    activePopover = null;
  }
}

async function loadInitialInputData(): Promise<void> {
  const activeSessionId = sidebarState.getActiveSessionId();
  const [models, context, generalSettings] = await Promise.all([
    getAvailableModelsIpc(),
    getContextUsageIpc(activeSessionId || undefined),
    getGeneralSettingsIpc().catch(() => null),
  ]);

  inputState.setAvailableModels(models);
  if (context) {
    inputState.setContextUsage(context);
  }
  if (generalSettings && typeof generalSettings.global_auto_approve_default === 'boolean') {
    inputState.setAutoApproveEnabled(generalSettings.global_auto_approve_default);
  }
  renderInputState();
}

function setupTextarea(): void {
  const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  if (!textarea) return;

  const autoResize = () => {
    textarea.style.height = 'auto';
    const newHeight = Math.min(200, Math.max(42, textarea.scrollHeight));
    textarea.style.height = `${newHeight}px`;
  };

  textarea.addEventListener('input', () => {
    inputState.setInputText(textarea.value);
    autoResize();
  });

  textarea.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (!inputState.getIsResponding()) {
        handleSubmit();
      }
    }
  });
}

function setupAttachButton(): void {
  document.getElementById('btn-attach-files')?.addEventListener('click', async () => {
    const picked = await pickAttachmentsIpc();
    if (picked.length > 0) {
      inputState.addAttachments(picked);
    }
  });
}

function setupAutoApproveButton(): void {
  const btn = document.getElementById('btn-auto-approve');
  btn?.addEventListener('click', async () => {
    const next = !inputState.isAutoApproveEnabled();
    inputState.setAutoApproveEnabled(next);
    renderInputState();
    try {
      await toggleAutoApproveIpc(next);
    } catch (err) {
      console.error('[Input] Failed to toggle auto approve:', err);
    }
  });
}

function setupModelSelector(): void {
  const btn = document.getElementById('btn-select-model');
  btn?.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleModelPopover(btn);
  });
}

function formatContextWindow(tokens: number): string {
  if (!tokens || tokens <= 0) return '';
  if (tokens >= 1_000_000) {
    const m = tokens / 1_000_000;
    return `${Number.isInteger(m) ? m : m.toFixed(1)}M`;
  }
  if (tokens >= 1_000) {
    return `${Math.round(tokens / 1_000)}k`;
  }
  return `${tokens}`;
}

function toggleModelPopover(trigger: HTMLElement): void {
  if (activePopover && activePopover.dataset.type === 'model') {
    dismissPopover();
    return;
  }
  dismissPopover();

  const models = inputState.getAvailableModels();
  const currentModel = inputState.getSelectedModel();
  const currentReasoning = inputState.getSelectedReasoning();

  const popover = document.createElement('div');
  popover.className = 'input-popover-dropdown';
  popover.dataset.type = 'model';

  // Header for the model popover
  const header = document.createElement('div');
  header.className = 'popover-dropdown-header';
  header.innerHTML = `
    <span class="ui-icon icon-input-model popover-header-icon"></span>
    <span class="popover-header-title">Select Model</span>
  `;
  popover.appendChild(header);

  const listContainer = document.createElement('div');
  listContainer.className = 'popover-items-list';

  models.forEach((m) => {
    const hasReasoning = Array.isArray(m.reasoning_levels) && m.reasoning_levels.length > 0;
    const isModelActive = m.id === currentModel;
    const ctxText = formatContextWindow(m.context_window);

    const item = document.createElement('div');
    item.className = `popover-item ${isModelActive ? 'active' : ''} ${hasReasoning ? 'has-submenu' : ''}`;

    if (hasReasoning) {
      item.innerHTML = `
        <div class="popover-item-main">
          <span class="popover-item-label" title="${m.id}">${m.name}</span>
        </div>
        <div class="popover-item-right">
          ${ctxText ? `<span class="popover-context-chip" title="Context capacity: ${m.context_window} tokens">${ctxText}</span>` : ''}
          <span class="popover-thinking-chip">Thinking</span>
          ${isModelActive ? '<span class="popover-active-check">✓</span>' : ''}
          <span class="ui-icon icon-sidebar-chevron-right popover-submenu-arrow"></span>
        </div>
      `;

      // Hover handler to open dynamic reasoning level sub-dropdown
      item.addEventListener('mouseenter', () => {
        if (subDropdownHoverTimeout !== null) {
          window.clearTimeout(subDropdownHoverTimeout);
          subDropdownHoverTimeout = null;
        }
        openReasoningSubDropdown(item, m, currentModel, currentReasoning);
      });

      item.addEventListener('mouseleave', () => {
        subDropdownHoverTimeout = window.setTimeout(() => {
          if (activeSubDropdown && !activeSubDropdown.matches(':hover') && !item.matches(':hover')) {
            dismissSubDropdown();
          }
        }, 150);
      });

      // Direct click on model item selects model with its active or default reasoning level
      item.addEventListener('click', async (evt) => {
        evt.stopPropagation();
        dismissPopover();
        const chosenReasoning = isModelActive && currentReasoning ? currentReasoning : m.reasoning_levels[0];
        await selectModelIpc(m.id, chosenReasoning, m.context_window);
        inputState.setSelectedModel(m.id);
        inputState.setSelectedReasoning(chosenReasoning);
        try {
          const updatedContext = await getContextUsageIpc();
          inputState.setContextUsage(updatedContext);
        } catch (err) {
          console.error('[Input] Failed to refresh context usage:', err);
        }
        renderInputState();
      });
    } else {
      item.innerHTML = `
        <div class="popover-item-main">
          <span class="popover-item-label" title="${m.id}">${m.name}</span>
        </div>
        <div class="popover-item-right">
          ${ctxText ? `<span class="popover-context-chip" title="Context capacity: ${m.context_window} tokens">${ctxText}</span>` : ''}
          ${isModelActive ? '<span class="popover-active-check">✓</span>' : ''}
        </div>
      `;

      item.addEventListener('mouseenter', () => {
        dismissSubDropdown();
      });

      item.addEventListener('click', async (evt) => {
        evt.stopPropagation();
        dismissPopover();
        await selectModelIpc(m.id, undefined, m.context_window);
        inputState.setSelectedModel(m.id);
        inputState.setSelectedReasoning('');
        try {
          const updatedContext = await getContextUsageIpc();
          inputState.setContextUsage(updatedContext);
        } catch (err) {
          console.error('[Input] Failed to refresh context usage:', err);
        }
        renderInputState();
      });
    }

    listContainer.appendChild(item);
  });

  popover.appendChild(listContainer);

  const rect = trigger.getBoundingClientRect();
  popover.style.bottom = `${window.innerHeight - rect.top + 6}px`;
  popover.style.right = `${window.innerWidth - rect.right}px`;

  document.body.appendChild(popover);
  activePopover = popover;
}

function openReasoningSubDropdown(
  parentItem: HTMLElement,
  model: ModelOption,
  currentModel: string,
  currentReasoning: string
): void {
  dismissSubDropdown();

  const subDropdown = document.createElement('div');
  subDropdown.className = 'input-sub-dropdown';

  const header = document.createElement('div');
  header.className = 'sub-dropdown-header';
  header.innerHTML = `
    <span class="ui-icon icon-input-reasoning sub-dropdown-header-icon"></span>
    <span>Reasoning Effort</span>
  `;
  subDropdown.appendChild(header);

  const list = document.createElement('div');
  list.className = 'sub-dropdown-list';

  model.reasoning_levels.forEach((level) => {
    const isLevelActive = model.id === currentModel && level === currentReasoning;
    const subItem = document.createElement('button');
    subItem.className = `popover-item sub-popover-item ${isLevelActive ? 'active' : ''}`;
    subItem.innerHTML = `
      <span class="popover-item-label">${level}</span>
      ${isLevelActive ? '<span class="popover-active-check">✓</span>' : ''}
    `;

    subItem.addEventListener('click', async (evt) => {
      evt.stopPropagation();
      dismissPopover();
      await selectModelIpc(model.id, level, model.context_window);
      inputState.setSelectedModel(model.id);
      inputState.setSelectedReasoning(level);
      try {
        const updatedContext = await getContextUsageIpc();
        inputState.setContextUsage(updatedContext);
      } catch (err) {
        console.error('[Input] Failed to refresh context usage:', err);
      }
      renderInputState();
    });

    list.appendChild(subItem);
  });

  subDropdown.appendChild(list);

  subDropdown.addEventListener('mouseenter', () => {
    if (subDropdownHoverTimeout !== null) {
      window.clearTimeout(subDropdownHoverTimeout);
      subDropdownHoverTimeout = null;
    }
  });

  subDropdown.addEventListener('mouseleave', () => {
    subDropdownHoverTimeout = window.setTimeout(() => {
      if (!parentItem.matches(':hover') && !subDropdown.matches(':hover')) {
        dismissSubDropdown();
      }
    }, 150);
  });

  document.body.appendChild(subDropdown);

  // Position sub-dropdown adjacent to parent item
  const itemRect = parentItem.getBoundingClientRect();
  const subRect = subDropdown.getBoundingClientRect();

  const spaceOnRight = window.innerWidth - itemRect.right;
  if (spaceOnRight >= subRect.width + 10) {
    subDropdown.style.left = `${itemRect.right + 6}px`;
  } else {
    subDropdown.style.right = `${window.innerWidth - itemRect.left + 6}px`;
  }

  const desiredTop = itemRect.top - 6;
  const maxTop = window.innerHeight - subRect.height - 12;
  subDropdown.style.top = `${Math.max(12, Math.min(desiredTop, maxTop))}px`;

  activeSubDropdown = subDropdown;
}

function setupVoiceButton(): void {
  const btn = document.getElementById('btn-voice-input');
  btn?.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleVoiceRecording();
  });
}

function setupSendButton(): void {
  document.getElementById('btn-send-message')?.addEventListener('click', () => {
    if (inputState.getIsResponding()) {
      handleCancel();
    } else {
      handleSubmit();
    }
  });
}

function setupOutsideClickListener(): void {
  window.addEventListener('click', () => {
    dismissPopover();
  });
}

async function handleSubmit(): Promise<void> {
  if (inputState.getIsVoiceRecording()) {
    stopVoiceRecording();
  }

  const text = inputState.getInputText().trim();
  const attachments = inputState.getPendingAttachments();

  if (!text && attachments.length === 0) return;

  const currentSessionId = sidebarState.getActiveSessionId();
  const workspacePath = sidebarState.getActiveProjectPath();

  const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  if (textarea) {
    textarea.value = '';
    textarea.style.height = 'auto';
  }

  inputState.setInputText('');
  inputState.clearAttachments();
  inputState.setIsResponding(true);

  // 1. Immediately insert user message into chat view
  const userTurnIndex = Math.floor(messagesState.getMessages().length / 2);
  const userMsg: ChatMessage = {
    id: `turn_${userTurnIndex}_user`,
    role: 'user',
    text,
    timestamp: 'Just now',
    created_at: Math.floor(Date.now() / 1000),
    turn_index: userTurnIndex,
    is_liked: false,
    is_disliked: false,
  };
  messagesState.addMessage(userMsg);

  // 2. Prepare streaming assistant placeholder
  messagesState.startAssistantStreaming(userTurnIndex);

  try {
    const activeId = await submitPromptIpc(
      currentSessionId,
      text,
      attachments,
      workspacePath
    );

    if (!currentSessionId && activeId) {
      sidebarState.selectSession(activeId, workspacePath);
    }
  } catch (err) {
    console.error('[Input] Failed to submit prompt:', err);
    inputState.setIsResponding(false);
    messagesState.finishStreaming();
  }
}

async function handleCancel(): Promise<void> {
  console.debug('[Input] Cancel prompt requested');
  await cancelPromptIpc();
  inputState.setIsResponding(false);
  messagesState.finishStreaming();
}

function renderInputState(): void {
  const isReadOnly = inputState.getIsReadOnly();
  const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  const attachBtn = document.getElementById('btn-attach-files') as HTMLButtonElement | null;
  const voiceBtn = document.getElementById('btn-voice-input') as HTMLButtonElement | null;

  if (textarea) {
    textarea.disabled = isReadOnly;
    if (isReadOnly) {
      textarea.placeholder = inputState.getReadOnlyReason() || 'Channel conversation (read-only in VS Code)';
    } else {
      textarea.placeholder = 'Ask a question or describe a task...';
    }
  }

  if (attachBtn) {
    attachBtn.disabled = isReadOnly;
    attachBtn.style.opacity = isReadOnly ? '0.35' : '1';
    attachBtn.style.pointerEvents = isReadOnly ? 'none' : 'auto';
  }

  if (voiceBtn) {
    voiceBtn.disabled = isReadOnly;
    voiceBtn.style.opacity = isReadOnly ? '0.35' : '1';
    voiceBtn.style.pointerEvents = isReadOnly ? 'none' : 'auto';
  }

  // 1. Attachments bar
  const bar = document.getElementById('input-attachments-bar');
  if (bar) {
    const attachments = inputState.getPendingAttachments();
    bar.classList.toggle('has-items', attachments.length > 0);
    bar.innerHTML = '';

    attachments.forEach((att, idx) => {
      const chip = document.createElement('div');
      chip.className = 'attachment-chip';
      chip.innerHTML = `
        <span class="session-title-text" title="${att.path}">${att.file_name}</span>
        <button class="attachment-chip-remove" title="Remove attachment">✕</button>
      `;

      chip.querySelector('.attachment-chip-remove')?.addEventListener('click', (e) => {
        e.stopPropagation();
        inputState.removeAttachment(idx);
      });

      bar.appendChild(chip);
    });
  }

  // 2. Auto-Approve button
  const autoBtn = document.getElementById('btn-auto-approve');
  if (autoBtn) {
    const enabled = inputState.isAutoApproveEnabled();
    autoBtn.classList.toggle('enabled', enabled);
    const icon = autoBtn.querySelector('.ui-icon');
    if (icon) {
      if (enabled) {
        icon.classList.remove('icon-input-auto-approve-disable');
        icon.classList.add('icon-input-auto-approve-enable');
      } else {
        icon.classList.remove('icon-input-auto-approve-enable');
        icon.classList.add('icon-input-auto-approve-disable');
      }
    }
  }

  // 3. Model & Reasoning selector unified pill text
  const modelText = document.getElementById('selected-model-text');
  const modelBtn = document.getElementById('btn-select-model');
  const activeModel = inputState.getActiveModelOption();
  const rawModelName = activeModel?.name || inputState.getSelectedModel() || 'Model';

  if (modelText) {
    // Truncate to first 20 characters and show ellipsis if longer for compact display
    const displayName = rawModelName.length > 20 ? `${rawModelName.slice(0, 20)}...` : rawModelName;
    modelText.textContent = displayName;
    modelText.title = rawModelName;
  }

  if (modelBtn) {
    const currentReasoning = inputState.getSelectedReasoning();
    modelBtn.title = currentReasoning && currentReasoning !== 'Disabled'
      ? `Model: ${rawModelName} (${currentReasoning})`
      : `Model: ${rawModelName}`;
  }

  const reasoningBadge = document.getElementById('selected-reasoning-badge');
  if (reasoningBadge) {
    const activeModel = inputState.getActiveModelOption();
    const currentReasoning = inputState.getSelectedReasoning();
    const hasReasoning = activeModel && Array.isArray(activeModel.reasoning_levels) && activeModel.reasoning_levels.length > 0;

    if (hasReasoning && currentReasoning && currentReasoning !== 'Disabled') {
      reasoningBadge.textContent = currentReasoning;
      reasoningBadge.style.display = 'inline-flex';
    } else {
      reasoningBadge.style.display = 'none';
      reasoningBadge.textContent = '';
    }
  }

  // 4. Context indicator
  const contextText = document.getElementById('context-usage-text');
  if (contextText) {
    contextText.textContent = inputState.getContextUsage().formatted;
  }

  // 6. Voice button recording state
  voiceBtn?.classList.toggle('recording', inputState.getIsVoiceRecording());

  // 7. Send button active / responding state
  const sendBtn = document.getElementById('btn-send-message');
  if (sendBtn) {
    const isResponding = inputState.getIsResponding();
    const hasContent =
      inputState.getInputText().trim().length > 0 || inputState.getPendingAttachments().length > 0;

    sendBtn.classList.toggle('responding', isResponding);
    sendBtn.classList.toggle('disabled', isReadOnly || (!isResponding && !hasContent));

    if (isReadOnly) {
      sendBtn.style.opacity = '0.35';
      sendBtn.style.pointerEvents = 'none';
    } else {
      sendBtn.style.opacity = '';
      sendBtn.style.pointerEvents = '';
    }

    const icon = sendBtn.querySelector('.ui-icon');
    if (icon) {
      if (isResponding) {
        icon.classList.remove('icon-input-send');
        icon.classList.add('icon-input-stop');
      } else {
        icon.classList.remove('icon-input-stop');
        icon.classList.add('icon-input-send');
      }
    }
  }
}
