// Compaction Pill Component & Expandable Summary DOM Renderer
//
// Renders an expandable pill in the message stream when context compaction occurs:
// - Header with compression icon, token delta badge (e.g. "Context compacted • 128.5k → 12.4k"), and chevron.
// - Expandable card rendering the markdown summary of all compacted turns.

import { renderMarkdownToHtml, postProcessMarkdownElement } from '../markdown/markdown.js';
import type { CompactionData } from './types.js';

function formatTokenCount(tokens: number): string {
  if (tokens >= 1_000_000) {
    const m = tokens / 1_000_000;
    return `${m % 1 === 0 ? m : m.toFixed(1)}M`;
  }
  if (tokens >= 1_000) {
    const k = tokens / 1_000;
    return `${k % 1 === 0 ? k : k.toFixed(1)}k`;
  }
  return `${tokens}`;
}

export function renderCompactionElement(
  data: CompactionData,
  onToggle: () => void
): HTMLElement {
  const container = document.createElement('div');
  container.className = `compaction-pill-container ${data.is_expanded ? 'expanded' : ''}`;

  // 1. Header Trigger Pill
  const header = document.createElement('div');
  header.className = 'compaction-pill-header';
  header.title = 'Click to view compacted conversation summary';

  // 1.1 Icon
  const iconSpan = document.createElement('span');
  iconSpan.className = 'compaction-pill-icon';
  iconSpan.innerHTML = `
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <path d="M4 14h6m-6-4h6m-6 8h16M14 4l-4 4 4 4m6-8l-4 4 4 4"/>
    </svg>
  `;

  // 1.2 Summary Text & Token Numbers
  const textSpan = document.createElement('span');
  textSpan.className = 'compaction-pill-text';

  const beforeStr = formatTokenCount(data.tokens_before);
  const afterStr = formatTokenCount(data.tokens_after);
  textSpan.textContent = `Context compacted • ${beforeStr} → ${afterStr}`;

  // 1.3 Percentage Saved Badge
  const savedTokens = Math.max(0, data.tokens_before - data.tokens_after);
  const savedPct = data.tokens_before > 0 ? Math.round((savedTokens / data.tokens_before) * 100) : 0;

  const badge = document.createElement('span');
  badge.className = 'compaction-saved-badge';
  badge.textContent = `-${savedPct}%`;

  // 1.4 Trailing Chevron
  const chevron = document.createElement('span');
  chevron.className = `compaction-pill-chevron ${data.is_expanded ? 'expanded' : ''}`;
  chevron.innerHTML = `
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="6 9 12 15 18 9"></polyline>
    </svg>
  `;

  header.appendChild(iconSpan);
  header.appendChild(textSpan);
  if (savedPct > 0) {
    header.appendChild(badge);
  }
  header.appendChild(chevron);

  header.addEventListener('click', (e) => {
    e.stopPropagation();
    onToggle();
  });

  container.appendChild(header);

  // 2. Expandable Summary Card Body
  const body = document.createElement('div');
  body.className = `compaction-pill-body ${data.is_expanded ? 'expanded' : ''}`;

  if (data.is_expanded) {
    const card = document.createElement('div');
    card.className = 'compaction-summary-card';

    const cardHeader = document.createElement('div');
    cardHeader.className = 'compaction-summary-header';
    cardHeader.textContent = 'Compacted Context Summary';

    const markdownBody = document.createElement('div');
    markdownBody.className = 'compaction-summary-content markdown-body';

    const rawSummary = data.summary || 'Context condensed into system snapshot.';
    renderMarkdownToHtml(rawSummary)
      .then((html) => {
        markdownBody.innerHTML = html;
        postProcessMarkdownElement(markdownBody);
      })
      .catch(() => {
        markdownBody.textContent = rawSummary;
      });

    card.appendChild(cardHeader);
    card.appendChild(markdownBody);
    body.appendChild(card);
  }

  container.appendChild(body);

  return container;
}

export function syncCompactionElement(
  existingEl: HTMLElement,
  data: CompactionData
): void {
  const isExpanded = !!data.is_expanded;
  existingEl.classList.toggle('expanded', isExpanded);

  const chevron = existingEl.querySelector('.compaction-pill-chevron');
  if (chevron) {
    chevron.classList.toggle('expanded', isExpanded);
  }

  let body = existingEl.querySelector('.compaction-pill-body') as HTMLElement | null;
  if (!body) {
    body = document.createElement('div');
    body.className = 'compaction-pill-body';
    existingEl.appendChild(body);
  }

  body.classList.toggle('expanded', isExpanded);

  if (isExpanded) {
    if (!body.querySelector('.compaction-summary-card')) {
      body.innerHTML = '';
      const card = document.createElement('div');
      card.className = 'compaction-summary-card';

      const cardHeader = document.createElement('div');
      cardHeader.className = 'compaction-summary-header';
      cardHeader.textContent = 'Compacted Context Summary';

      const markdownBody = document.createElement('div');
      markdownBody.className = 'compaction-summary-content markdown-body';

      const rawSummary = data.summary || 'Context condensed into system snapshot.';
      renderMarkdownToHtml(rawSummary)
        .then((html) => {
          markdownBody.innerHTML = html;
          postProcessMarkdownElement(markdownBody);
        })
        .catch(() => {
          markdownBody.textContent = rawSummary;
        });

      card.appendChild(cardHeader);
      card.appendChild(markdownBody);
      body.appendChild(card);
    }
  } else {
    body.innerHTML = '';
  }
}
