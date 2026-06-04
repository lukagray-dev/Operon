'use strict';

/**
 * toast.js — Minimal DOM toast notification system.
 *
 * Provides showSuccess() and showError() used by the settings modules.
 * Toasts appear at the bottom-right of the screen and auto-dismiss.
 * No external dependencies.
 */

// ── Styles (injected once into <head>) ───────────────────────────────────────

const TOAST_STYLES = `
  .toast-container {
    position: fixed;
    bottom: 24px;
    right: 24px;
    z-index: 99999;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-radius: 8px;
    font-size: 13px;
    font-family: inherit;
    color: #e8e8e8;
    background: #1e1e1e;
    border: 1px solid rgba(255,255,255,0.10);
    box-shadow: 0 4px 20px rgba(0,0,0,0.5);
    pointer-events: auto;
    opacity: 0;
    transform: translateY(8px);
    transition: opacity 180ms ease, transform 180ms ease;
    max-width: 320px;
    word-break: break-word;
  }
  .toast.is-visible {
    opacity: 1;
    transform: translateY(0);
  }
  .toast--success { border-left: 3px solid #22c55e; }
  .toast--error   { border-left: 3px solid #ef4444; }
  .toast__dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .toast--success .toast__dot { background: #22c55e; }
  .toast--error   .toast__dot { background: #ef4444; }
`;

// ── Internal state ────────────────────────────────────────────────────────────

let container = null;
let stylesInjected = false;

/** Injects CSS and creates the container element once. */
function ensureSetup() {
  if (!stylesInjected) {
    const style = document.createElement('style');
    style.textContent = TOAST_STYLES;
    document.head.appendChild(style);
    stylesInjected = true;
  }
  if (!container) {
    container = document.createElement('div');
    container.className = 'toast-container';
    container.setAttribute('aria-live', 'polite');
    container.setAttribute('aria-atomic', 'false');
    document.body.appendChild(container);
  }
}

/**
 * Shows a toast notification.
 * @param {string}  message  - Text to display.
 * @param {'success'|'error'} type - Visual style.
 * @param {number}  [duration=3000]  - Auto-dismiss delay in milliseconds.
 */
function showToast(message, type, duration = 3000) {
  ensureSetup();

  // Build toast element
  const toast = document.createElement('div');
  toast.className = `toast toast--${type}`;
  toast.setAttribute('role', type === 'error' ? 'alert' : 'status');
  toast.innerHTML = `
    <span class="toast__dot" aria-hidden="true"></span>
    <span>${String(message)}</span>
  `;

  container.appendChild(toast);

  // Trigger entrance animation on next frame
  requestAnimationFrame(() => {
    requestAnimationFrame(() => toast.classList.add('is-visible'));
  });

  // Auto-dismiss after duration
  setTimeout(() => {
    toast.classList.remove('is-visible');
    // Remove from DOM after transition completes
    toast.addEventListener('transitionend', () => toast.remove(), { once: true });
  }, duration);
}

/**
 * Shows a green success toast.
 * @param {string} message
 */
export function showSuccess(message) {
  showToast(message, 'success');
}

/**
 * Shows a red error toast.
 * @param {string} message
 */
export function showError(message) {
  showToast(message, 'error', 5000);
}
