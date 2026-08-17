// Custom Modal Dialog System (Confirm, Alert, Prompt, Permission)
//
// Replaces default native browser/WebView dialogs (confirm, alert, prompt, permission prompts)
// with consistent, dark-themed, glassmorphism modal dialogs matching Operon's design system.

export interface ConfirmDialogOptions {
  title?: string;
  message: string;
  confirmText?: string; // Defaults to "Ok" (or "Delete" if isDanger is true)
  cancelText?: string;  // Defaults to "Cancel"
  isDanger?: boolean;   // If true, confirm button has red/destructive highlight
  icon?: 'trash' | 'warning' | 'info' | 'help' | 'mic';
}

export interface AlertDialogOptions {
  title?: string;
  message: string;
  buttonText?: string; // Defaults to "Ok"
  icon?: 'info' | 'warning' | 'danger';
}

export interface PromptDialogOptions {
  title?: string;
  message?: string;
  defaultValue?: string;
  placeholder?: string;
  confirmText?: string; // Defaults to "Ok"
  cancelText?: string;  // Defaults to "Cancel"
  icon?: 'pencil' | 'help' | 'info';
}

export interface PermissionDialogOptions {
  title?: string;
  message: string;
  allowText?: string; // Defaults to "Allow"
  denyText?: string;  // Defaults to "Cancel"
  icon?: 'mic' | 'info' | 'warning';
}

/**
 * Displays a custom confirmation modal with "Ok" and "Cancel" buttons.
 * Returns a Promise that resolves to `true` if confirmed, or `false` if cancelled.
 */
export function showConfirmDialog(options: ConfirmDialogOptions | string): Promise<boolean> {
  const opts: ConfirmDialogOptions =
    typeof options === 'string'
      ? { message: options }
      : options;

  const title = opts.title || (opts.isDanger ? 'Confirmation' : 'Confirm Action');
  const confirmText = opts.confirmText || (opts.isDanger ? 'Delete' : 'Ok');
  const cancelText = opts.cancelText || 'Cancel';
  const iconType = opts.icon || (opts.isDanger ? 'trash' : 'warning');

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const card = document.createElement('div');
    card.className = 'dialog-card';

    card.innerHTML = `
      <div class="dialog-body">
        <div class="dialog-header">
          <div class="dialog-icon-badge ${opts.isDanger ? 'danger' : 'warning'}">
            <span class="ui-icon ${iconType === 'trash' ? 'icon-dialog-trash' : 'icon-dialog-warning'}"></span>
          </div>
          <div class="dialog-text-content">
            <h3 class="dialog-title">${escapeHtml(title)}</h3>
            <p class="dialog-message">${escapeHtml(opts.message)}</p>
          </div>
        </div>
      </div>
      <div class="dialog-footer">
        <button class="dialog-btn dialog-btn-cancel" id="dialog-btn-cancel">${escapeHtml(cancelText)}</button>
        <button class="dialog-btn dialog-btn-confirm ${opts.isDanger ? 'danger' : ''}" id="dialog-btn-confirm">${escapeHtml(confirmText)}</button>
      </div>
    `;

    overlay.appendChild(card);
    document.body.appendChild(overlay);

    const confirmBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-confirm');
    const cancelBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-cancel');

    // Auto-focus the confirm button by default
    confirmBtn?.focus();

    const cleanup = (result: boolean) => {
      window.removeEventListener('keydown', handleKeydown);
      overlay.remove();
      resolve(result);
    };

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cleanup(false);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        cleanup(true);
      }
    };

    window.addEventListener('keydown', handleKeydown);

    confirmBtn?.addEventListener('click', () => cleanup(true));
    cancelBtn?.addEventListener('click', () => cleanup(false));

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        cleanup(false);
      }
    });
  });
}

/**
 * Displays a custom alert modal with a single "Ok" button.
 * Returns a Promise that resolves when acknowledged.
 */
export function showAlertDialog(options: AlertDialogOptions | string): Promise<void> {
  const opts: AlertDialogOptions =
    typeof options === 'string'
      ? { message: options }
      : options;

  const title = opts.title || 'Notice';
  const buttonText = opts.buttonText || 'Ok';
  const iconType = opts.icon || 'info';

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const card = document.createElement('div');
    card.className = 'dialog-card';

    card.innerHTML = `
      <div class="dialog-body">
        <div class="dialog-header">
          <div class="dialog-icon-badge ${iconType}">
            <span class="ui-icon ${iconType === 'warning' ? 'icon-dialog-warning' : 'icon-dialog-info'}"></span>
          </div>
          <div class="dialog-text-content">
            <h3 class="dialog-title">${escapeHtml(title)}</h3>
            <p class="dialog-message">${escapeHtml(opts.message)}</p>
          </div>
        </div>
      </div>
      <div class="dialog-footer">
        <button class="dialog-btn dialog-btn-confirm" id="dialog-btn-ok">${escapeHtml(buttonText)}</button>
      </div>
    `;

    overlay.appendChild(card);
    document.body.appendChild(overlay);

    const okBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-ok');
    okBtn?.focus();

    const cleanup = () => {
      window.removeEventListener('keydown', handleKeydown);
      overlay.remove();
      resolve();
    };

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' || e.key === 'Enter') {
        e.preventDefault();
        cleanup();
      }
    };

    window.addEventListener('keydown', handleKeydown);
    okBtn?.addEventListener('click', () => cleanup());

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        cleanup();
      }
    });
  });
}

/**
 * Displays a custom prompt modal with a text input field, "Ok" and "Cancel" buttons.
 * Returns a Promise that resolves with the entered string, or `null` if cancelled.
 */
export function showPromptDialog(options: PromptDialogOptions | string): Promise<string | null> {
  const opts: PromptDialogOptions =
    typeof options === 'string'
      ? { message: options }
      : options;

  const title = opts.title || 'Enter Value';
  const confirmText = opts.confirmText || 'Ok';
  const cancelText = opts.cancelText || 'Cancel';
  const defaultValue = opts.defaultValue || '';

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const card = document.createElement('div');
    card.className = 'dialog-card';

    card.innerHTML = `
      <div class="dialog-body">
        <div class="dialog-header">
          <div class="dialog-icon-badge info">
            <span class="ui-icon icon-dialog-pencil"></span>
          </div>
          <div class="dialog-text-content">
            <h3 class="dialog-title">${escapeHtml(title)}</h3>
            ${opts.message ? `<p class="dialog-message">${escapeHtml(opts.message)}</p>` : ''}
          </div>
        </div>
        <div class="dialog-input-container">
          <input
            type="text"
            class="dialog-input"
            id="dialog-prompt-input"
            value="${escapeHtml(defaultValue)}"
            placeholder="${escapeHtml(opts.placeholder || '')}"
            spellcheck="false"
            autocomplete="off"
          />
        </div>
      </div>
      <div class="dialog-footer">
        <button class="dialog-btn dialog-btn-cancel" id="dialog-btn-cancel">${escapeHtml(cancelText)}</button>
        <button class="dialog-btn dialog-btn-confirm" id="dialog-btn-confirm">${escapeHtml(confirmText)}</button>
      </div>
    `;

    overlay.appendChild(card);
    document.body.appendChild(overlay);

    const input = card.querySelector<HTMLInputElement>('#dialog-prompt-input');
    const confirmBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-confirm');
    const cancelBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-cancel');

    // Focus input and select entire text for quick editing
    if (input) {
      input.focus();
      input.select();
    }

    const cleanup = (val: string | null) => {
      window.removeEventListener('keydown', handleKeydown);
      overlay.remove();
      resolve(val);
    };

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cleanup(null);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        cleanup(input ? input.value : '');
      }
    };

    window.addEventListener('keydown', handleKeydown);

    confirmBtn?.addEventListener('click', () => {
      cleanup(input ? input.value : '');
    });

    cancelBtn?.addEventListener('click', () => {
      cleanup(null);
    });

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        cleanup(null);
      }
    });
  });
}

/**
 * Displays a custom permission request modal (e.g. for Microphone access) with "Allow" and "Cancel" buttons.
 * Returns a Promise that resolves to `true` if allowed, or `false` if denied/cancelled.
 */
export function showPermissionDialog(options: PermissionDialogOptions): Promise<boolean> {
  const title = options.title || 'Permission Required';
  const allowText = options.allowText || 'Ok';
  const denyText = options.denyText || 'Cancel';
  const iconType = options.icon || 'mic';

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const card = document.createElement('div');
    card.className = 'dialog-card';

    card.innerHTML = `
      <div class="dialog-body">
        <div class="dialog-header">
          <div class="dialog-icon-badge primary">
            <span class="ui-icon ${iconType === 'mic' ? 'icon-dialog-mic' : 'icon-dialog-info'}"></span>
          </div>
          <div class="dialog-text-content">
            <h3 class="dialog-title">${escapeHtml(title)}</h3>
            <p class="dialog-message">${escapeHtml(options.message)}</p>
          </div>
        </div>
      </div>
      <div class="dialog-footer">
        <button class="dialog-btn dialog-btn-cancel" id="dialog-btn-deny">${escapeHtml(denyText)}</button>
        <button class="dialog-btn dialog-btn-confirm" id="dialog-btn-allow">${escapeHtml(allowText)}</button>
      </div>
    `;

    overlay.appendChild(card);
    document.body.appendChild(overlay);

    const allowBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-allow');
    const denyBtn = card.querySelector<HTMLButtonElement>('#dialog-btn-deny');

    allowBtn?.focus();

    const cleanup = (granted: boolean) => {
      window.removeEventListener('keydown', handleKeydown);
      overlay.remove();
      resolve(granted);
    };

    const handleKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cleanup(false);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        cleanup(true);
      }
    };

    window.addEventListener('keydown', handleKeydown);

    allowBtn?.addEventListener('click', () => cleanup(true));
    denyBtn?.addEventListener('click', () => cleanup(false));

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        cleanup(false);
      }
    });
  });
}

/**
 * Escapes HTML characters to prevent XSS.
 */
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
