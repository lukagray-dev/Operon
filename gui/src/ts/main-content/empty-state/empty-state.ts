// Empty State Controller & DOM Coordinator

import { emptyState } from './state.js';

export function initEmptyState(): void {
  // Re-render when empty state visibility changes
  emptyState.subscribe(() => {
    renderEmptyState();
  });

  renderEmptyState();
}

export function setEmptyStateVisible(visible: boolean): void {
  emptyState.setVisible(visible);
}

function renderEmptyState(): void {
  const el = document.getElementById('chat-empty-state');
  if (el) {
    el.classList.toggle('hidden', !emptyState.isVisible());
  }
}
