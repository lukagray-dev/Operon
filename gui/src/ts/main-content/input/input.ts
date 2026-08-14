// Main Content Input Panel Controller & DOM Coordinator

import {
  getAvailableModelsIpc,
  getContextUsageIpc,
  pickAttachmentsIpc,
  selectModelIpc,
  toggleAutoApproveIpc,
} from './ipc.js';
import { inputState } from './state.js';
import type { ReasoningLevel } from './types.js';

let activePopover: HTMLElement | null = null;

export function initInputPanel(): void {
  setupTextarea();
  setupAttachButton();
  setupAutoApproveButton();
  setupModelSelector();
  setupReasoningSelector();
  setupVoiceButton();
  setupSendButton();
  setupOutsideClickListener();

  // Initial load
  loadInitialInputData();

  // Re-render when input state changes
  inputState.subscribe(() => {
    renderInputState();
  });
}

export function dismissPopover(): void {
  if (activePopover) {
    activePopover.remove();
    activePopover = null;
  }
}

async function loadInitialInputData(): Promise<void> {
  const [models, context] = await Promise.all([
    getAvailableModelsIpc(),
    getContextUsageIpc(),
  ]);

  inputState.setAvailableModels(models);
  inputState.setContextUsage(context);
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
      handleSubmit();
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
    const current = inputState.isAutoApproveEnabled();
    const next = await toggleAutoApproveIpc(!current);
    inputState.setAutoApproveEnabled(next);
  });
}

function setupModelSelector(): void {
  const btn = document.getElementById('btn-select-model');
  btn?.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleModelPopover(btn);
  });
}

function toggleModelPopover(trigger: HTMLElement): void {
  if (activePopover && activePopover.dataset.type === 'model') {
    dismissPopover();
    return;
  }
  dismissPopover();

  const models = inputState.getAvailableModels();
  const current = inputState.getSelectedModel();

  const popover = document.createElement('div');
  popover.className = 'input-popover-dropdown';
  popover.dataset.type = 'model';

  models.forEach((m) => {
    const item = document.createElement('button');
    item.className = `popover-item ${m.id === current ? 'active' : ''}`;
    item.innerHTML = `
      <span class="popover-item-label" title="${m.id}">${m.name}</span>
      ${m.id === current ? '<span style="font-size: 11px; opacity: 0.7;">✓</span>' : ''}
    `;

    item.addEventListener('click', async (evt) => {
      evt.stopPropagation();
      dismissPopover();
      await selectModelIpc(m.id);
      inputState.setSelectedModel(m.id);
    });

    popover.appendChild(item);
  });

  const rect = trigger.getBoundingClientRect();
  popover.style.bottom = `${window.innerHeight - rect.top + 4}px`;
  popover.style.right = `${window.innerWidth - rect.right}px`;

  document.body.appendChild(popover);
  activePopover = popover;
}

function setupReasoningSelector(): void {
  const btn = document.getElementById('btn-select-reasoning');
  btn?.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleReasoningPopover(btn);
  });
}

function toggleReasoningPopover(trigger: HTMLElement): void {
  if (activePopover && activePopover.dataset.type === 'reasoning') {
    dismissPopover();
    return;
  }
  dismissPopover();

  const levels: ReasoningLevel[] = ['Low', 'Medium', 'High', 'Disabled'];
  const current = inputState.getSelectedReasoning();

  const popover = document.createElement('div');
  popover.className = 'input-popover-dropdown';
  popover.dataset.type = 'reasoning';

  levels.forEach((lvl) => {
    const item = document.createElement('button');
    item.className = `popover-item ${lvl === current ? 'active' : ''}`;
    item.innerHTML = `
      <span>${lvl}</span>
      ${lvl === current ? '<span style="font-size: 11px; opacity: 0.7;">✓</span>' : ''}
    `;

    item.addEventListener('click', (evt) => {
      evt.stopPropagation();
      dismissPopover();
      inputState.setSelectedReasoning(lvl);
    });

    popover.appendChild(item);
  });

  const rect = trigger.getBoundingClientRect();
  popover.style.bottom = `${window.innerHeight - rect.top + 4}px`;
  popover.style.right = `${window.innerWidth - rect.right}px`;

  document.body.appendChild(popover);
  activePopover = popover;
}

function setupVoiceButton(): void {
  const btn = document.getElementById('btn-voice-input');
  btn?.addEventListener('click', () => {
    const current = inputState.getIsVoiceRecording();
    inputState.setIsVoiceRecording(!current);
    console.debug('[Input] Voice toggled:', !current);
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

function handleSubmit(): void {
  const text = inputState.getInputText().trim();
  const attachments = inputState.getPendingAttachments();

  if (text.length === 0 && attachments.length === 0) return;

  console.debug('[Input] Submitting prompt:', text, 'with attachments:', attachments.length);

  // Clear input
  const textarea = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  if (textarea) {
    textarea.value = '';
    textarea.style.height = 'auto';
  }

  inputState.setInputText('');
  inputState.clearAttachments();
}

function handleCancel(): void {
  console.debug('[Input] Cancel prompt requested');
  inputState.setIsResponding(false);
}

function renderInputState(): void {
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

  // 3. Model selector button text
  const modelText = document.getElementById('selected-model-text');
  if (modelText) {
    modelText.textContent = inputState.getSelectedModel();
  }

  // 4. Reasoning selector button text
  const reasoningText = document.getElementById('selected-reasoning-text');
  if (reasoningText) {
    reasoningText.textContent = inputState.getSelectedReasoning();
  }

  // 5. Context indicator
  const contextText = document.getElementById('context-usage-text');
  if (contextText) {
    contextText.textContent = inputState.getContextUsage().formatted;
  }

  // 6. Voice button recording state
  const voiceBtn = document.getElementById('btn-voice-input');
  voiceBtn?.classList.toggle('recording', inputState.getIsVoiceRecording());

  // 7. Send button active / responding state
  const sendBtn = document.getElementById('btn-send-message');
  if (sendBtn) {
    const isResponding = inputState.getIsResponding();
    const hasContent =
      inputState.getInputText().trim().length > 0 || inputState.getPendingAttachments().length > 0;

    sendBtn.classList.toggle('responding', isResponding);
    sendBtn.classList.toggle('disabled', !isResponding && !hasContent);

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
