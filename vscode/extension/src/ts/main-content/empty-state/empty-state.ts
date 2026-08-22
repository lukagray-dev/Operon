// ============================================================================
// Empty State View Controller
//
// Hey friend! This module controls the splash / tagline hero screen shown
// when no active conversation stream is present.
// ============================================================================

let emptyStateEl: HTMLElement | null = null;
let messagesContainerEl: HTMLElement | null = null;

/**
 * Toggles visibility between the centered empty state hero and the message list.
 */
export function setEmptyStateVisible(visible: boolean): void {
  if (!emptyStateEl) emptyStateEl = document.getElementById('chat-empty-state');
  if (!messagesContainerEl) messagesContainerEl = document.getElementById('messages-container');

  if (visible) {
    emptyStateEl?.classList.remove('hidden');
    messagesContainerEl?.classList.add('hidden');
  } else {
    emptyStateEl?.classList.add('hidden');
    messagesContainerEl?.classList.remove('hidden');
  }
}

/**
 * Initializes empty state DOM references.
 */
export function initEmptyState(): void {
  emptyStateEl = document.getElementById('chat-empty-state');
  messagesContainerEl = document.getElementById('messages-container');
  console.log('[Operon EmptyState] Initialized.');
}
