// Interactive Clarification Prompt Card DOM Component
//
// Hey friend! This module handles the interactive UI card rendered whenever the agent
// calls the `ask` tool to request clarifying information from the user. It presents
// 3 clickable option choices plus a free-text custom answer field.

import type { AskQuestionData } from './types.js';

/**
 * Escapes raw strings for safe insertion into HTML.
 */
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Creates the DOM element for an Ask Question prompt card.
 *
 * @param data The prompt data (id, question, options, answered status)
 * @param onAnswer Async callback invoked when the user selects or submits an answer
 * @returns The fully constructed HTMLElement
 */
export function createAskCardElement(
  data: AskQuestionData,
  onAnswer: (answer: string) => Promise<void>
): HTMLElement {
  const card = document.createElement('div');
  card.className = 'ask-card';
  card.setAttribute('data-ask-id', data.id);

  // 1. Header with icon and title
  const header = document.createElement('div');
  header.className = 'ask-card-header';
  header.innerHTML = `
    <div class="ask-card-icon">
      <img src="assets/icons/main-content/messages/ask-question.svg" alt="Question" />
    </div>
    <span class="ask-card-title">Clarification Requested</span>
  `;
  card.appendChild(header);

  // 2. Card Body
  const body = document.createElement('div');
  body.className = 'ask-card-body';

  // 2.1 Question text
  const questionEl = document.createElement('div');
  questionEl.className = 'ask-card-question';
  questionEl.textContent = data.question;
  body.appendChild(questionEl);

  if (data.is_answered && data.answer) {
    // If this card is already answered (e.g. reloaded from session history), render answered state
    const statusEl = document.createElement('div');
    statusEl.className = 'ask-card-status';
    statusEl.innerHTML = `
      <svg class="ask-card-check-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"></polyline>
      </svg>
      <span>Answered: <strong>${escapeHtml(data.answer)}</strong></span>
    `;
    body.appendChild(statusEl);
  } else {
    // 2.2 Active Options (3 predefined choices)
    const optionsContainer = document.createElement('div');
    optionsContainer.className = 'ask-card-options';

    const submitAnswer = async (selectedAnswer: string) => {
      const trimmed = selectedAnswer.trim();
      if (!trimmed) return;

      // Disable all buttons and input to prevent duplicate requests
      card.querySelectorAll<HTMLButtonElement | HTMLInputElement>(
        '.btn-ask-option, .btn-ask-submit, .input-ask-custom'
      ).forEach((el) => {
        el.disabled = true;
      });

      try {
        await onAnswer(trimmed);
        resolveAskCardElement(card, trimmed);
      } catch (err) {
        console.error('[AskCard] Failed to submit answer:', err);
        // Re-enable on error so user can retry
        card.querySelectorAll<HTMLButtonElement | HTMLInputElement>(
          '.btn-ask-option, .btn-ask-submit, .input-ask-custom'
        ).forEach((el) => {
          el.disabled = false;
        });
      }
    };

    data.options.forEach((optionText) => {
      const btn = document.createElement('button');
      btn.className = 'btn-ask-option';
      btn.textContent = optionText;
      btn.addEventListener('click', () => {
        submitAnswer(optionText);
      });
      optionsContainer.appendChild(btn);
    });

    body.appendChild(optionsContainer);

    // 2.3 Custom Answer Input Row
    const customRow = document.createElement('div');
    customRow.className = 'ask-card-custom';

    const customInput = document.createElement('input');
    customInput.type = 'text';
    customInput.className = 'input-ask-custom';
    customInput.placeholder = 'Or type a custom answer...';

    const submitBtn = document.createElement('button');
    submitBtn.className = 'btn-ask-submit';
    submitBtn.textContent = 'Submit';

    const handleCustomSubmit = () => {
      const customVal = customInput.value.trim();
      if (customVal) {
        submitAnswer(customVal);
      }
    };

    submitBtn.addEventListener('click', handleCustomSubmit);
    customInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleCustomSubmit();
      }
    });

    customRow.appendChild(customInput);
    customRow.appendChild(submitBtn);
    body.appendChild(customRow);
  }

  card.appendChild(body);
  return card;
}

/**
 * Transitions an active Ask Question card into the answered state in-place.
 *
 * @param cardEl The ask card container element
 * @param answer The answer string that was submitted
 */
export function resolveAskCardElement(cardEl: HTMLElement, answer: string): void {
  const body = cardEl.querySelector('.ask-card-body');
  if (!body) return;

  // Remove options container and custom input row
  const optionsEl = body.querySelector('.ask-card-options');
  if (optionsEl) optionsEl.remove();

  const customEl = body.querySelector('.ask-card-custom');
  if (customEl) customEl.remove();

  // Insert answered status if not already present
  if (!body.querySelector('.ask-card-status')) {
    const statusEl = document.createElement('div');
    statusEl.className = 'ask-card-status';
    statusEl.innerHTML = `
      <svg class="ask-card-check-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="20 6 9 17 4 12"></polyline>
      </svg>
      <span>Answered: <strong>${escapeHtml(answer)}</strong></span>
    `;
    body.appendChild(statusEl);
  }
}
