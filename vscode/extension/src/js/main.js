// ============================================================================
// Operon Webview Root Coordinator
//
// Hey friend! This is the main entrypoint running inside our Webview DOM.
// It initializes and coordinates the UI layout:
// 1. Left Sidebar (collapsible overlay drawer matching GUI structure)
// 2. Three-dots Conversation Context Menu Dropdown
// 3. Settings Button & Global Shortcut (`Ctrl+,`) opening Settings Editor Tab
// 4. Topbar with hamburger / sidebar toggle button
// 5. Central chat viewport with Empty State / Streaming Messages
// 6. User Message Bubbles (with Copy & Inline Edit actions)
// 7. Assistant Responses (with Copy/Like/Dislike/Fork actions)
// 8. Pinned bottom floating input card with auto-resizing textarea
// ============================================================================
import { invokeIpc } from './shared/ipc.js';
window.addEventListener('DOMContentLoaded', () => {
    console.log('[Operon Webview] Initializing UI layout...');
    // DOM Elements
    const btnToggleSidebar = document.getElementById('btn-toggle-sidebar');
    const btnCloseSidebar = document.getElementById('btn-close-sidebar');
    const sidebarBackdrop = document.getElementById('sidebar-backdrop');
    const leftSidebar = document.getElementById('left-sidebar');
    const btnSidebarSettings = document.getElementById('btn-sidebar-settings');
    const inputPrompt = document.getElementById('chat-input-textarea');
    const btnSend = document.getElementById('btn-send-message');
    const btnAutoApprove = document.getElementById('btn-auto-approve');
    const searchInput = document.getElementById('sidebar-search-input');
    const btnSearchClear = document.getElementById('btn-search-clear');
    const chatViewport = document.getElementById('chat-messages-viewport');
    const chatEmptyState = document.getElementById('chat-empty-state');
    const messagesContainer = document.getElementById('messages-container');
    // ── Overlay Sidebar Drawer Logic ──────────────────────────────────────────
    const openSidebar = () => {
        leftSidebar?.classList.add('open');
        sidebarBackdrop?.classList.add('visible');
        const toggleIcon = btnToggleSidebar?.querySelector('.ui-icon');
        if (toggleIcon) {
            toggleIcon.className = 'ui-icon icon-titlebar-sidebar-opened';
        }
    };
    const closeSidebar = () => {
        leftSidebar?.classList.remove('open');
        sidebarBackdrop?.classList.remove('visible');
        dismissContextMenu();
        const toggleIcon = btnToggleSidebar?.querySelector('.ui-icon');
        if (toggleIcon) {
            toggleIcon.className = 'ui-icon icon-titlebar-sidebar-closed';
        }
    };
    // Toggle on hamburger click
    btnToggleSidebar?.addEventListener('click', () => {
        if (leftSidebar?.classList.contains('open')) {
            closeSidebar();
        }
        else {
            openSidebar();
        }
    });
    // Close when clicking close (✕) button or dimmed backdrop
    btnCloseSidebar?.addEventListener('click', closeSidebar);
    sidebarBackdrop?.addEventListener('click', closeSidebar);
    // Global keydown: Escape to close sidebar / dropdown, Ctrl+, to open settings
    window.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            if (activeContextMenu) {
                dismissContextMenu();
            }
            else if (leftSidebar?.classList.contains('open')) {
                closeSidebar();
            }
        }
        else if ((e.ctrlKey || e.metaKey) && e.key === ',') {
            e.preventDefault();
            invokeIpc('open_settings_window');
        }
    });
    // Open Settings on Sidebar Button Click
    btnSidebarSettings?.addEventListener('click', () => {
        closeSidebar();
        invokeIpc('open_settings_window');
    });
    // Section Accordion Header Collapse / Expand
    document.querySelectorAll('.section-header').forEach((header) => {
        header.addEventListener('click', (e) => {
            if (e.target.closest('.section-action-btn')) {
                return;
            }
            const section = header.closest('.sidebar-section');
            section?.classList.toggle('collapsed');
        });
    });
    // Project Header Collapse / Expand
    document.querySelectorAll('.project-header').forEach((header) => {
        header.addEventListener('click', (e) => {
            if (e.target.closest('.item-more-btn')) {
                return;
            }
            const card = header.closest('.project-card');
            card?.classList.toggle('collapsed');
        });
    });
    // Search Input Clear Button
    if (searchInput && btnSearchClear) {
        searchInput.addEventListener('input', () => {
            btnSearchClear.classList.toggle('visible', searchInput.value.length > 0);
        });
        btnSearchClear.addEventListener('click', () => {
            searchInput.value = '';
            btnSearchClear.classList.remove('visible');
            searchInput.focus();
        });
    }
    // ── Conversation Item Three Dots Context Menu Dropdown ───────────────────
    let activeContextMenu = null;
    function dismissContextMenu() {
        if (activeContextMenu) {
            activeContextMenu.remove();
            activeContextMenu = null;
        }
        document.querySelectorAll('.item-more-btn.active').forEach((btn) => {
            btn.classList.remove('active');
        });
    }
    function showContextMenu(e, targetBtn) {
        e.stopPropagation();
        dismissContextMenu();
        targetBtn.classList.add('active');
        const rect = targetBtn.getBoundingClientRect();
        const menu = document.createElement('div');
        menu.className = 'session-context-menu';
        const top = Math.min(window.innerHeight - 200, rect.bottom + 4);
        const left = Math.min(window.innerWidth - 150, rect.left - 100);
        menu.style.top = `${Math.max(8, top)}px`;
        menu.style.left = `${Math.max(8, left)}px`;
        menu.innerHTML = `
      <button class="context-menu-item" id="ctx-share">
        <span class="ui-icon icon-sidebar-share"></span>
        <span>Share</span>
      </button>
      <button class="context-menu-item" id="ctx-rename">
        <span class="ui-icon icon-sidebar-pencil"></span>
        <span>Rename</span>
      </button>
      <button class="context-menu-item" id="ctx-move">
        <span class="ui-icon icon-sidebar-folder-input"></span>
        <span>Move to...</span>
      </button>
      <button class="context-menu-item" id="ctx-fork">
        <span class="ui-icon icon-sidebar-fork"></span>
        <span>Fork</span>
      </button>
      <div class="context-menu-separator"></div>
      <button class="context-menu-item danger" id="ctx-delete">
        <span class="ui-icon icon-sidebar-trash"></span>
        <span>Delete</span>
      </button>
    `;
        document.body.appendChild(menu);
        activeContextMenu = menu;
        menu.querySelectorAll('.context-menu-item').forEach((item) => {
            item.addEventListener('click', (ev) => {
                ev.stopPropagation();
                const actionId = item.id;
                console.log(`[Operon Sidebar] Context menu action: ${actionId}`);
                dismissContextMenu();
            });
        });
    }
    // Bind three-dots buttons
    document.querySelectorAll('.item-more-btn').forEach((btn) => {
        btn.addEventListener('click', (e) => {
            showContextMenu(e, btn);
        });
    });
    // Global dismiss on outside click
    window.addEventListener('click', (e) => {
        if (activeContextMenu && !e.target.closest('.session-context-menu')) {
            dismissContextMenu();
        }
    });
    // ── Helper: Scroll Chat Viewport to Bottom ─────────────────────────────────
    const scrollToBottom = () => {
        if (chatViewport) {
            chatViewport.scrollTo({
                top: chatViewport.scrollHeight,
                behavior: 'smooth',
            });
        }
    };
    // ── Helper: Escape HTML string ─────────────────────────────────────────────
    const escapeHtml = (text) => {
        const div = document.createElement('div');
        div.innerText = text;
        return div.innerHTML;
    };
    // ── Helper: Copy to Clipboard with Button Icon Feedback ────────────────────
    const copyWithFeedback = async (text, iconElem, checkClass, originalClass) => {
        try {
            await navigator.clipboard.writeText(text);
            if (iconElem) {
                iconElem.className = `ui-icon ${checkClass}`;
                setTimeout(() => {
                    iconElem.className = `ui-icon ${originalClass}`;
                }, 1500);
            }
        }
        catch (err) {
            console.error('Failed to copy text:', err);
        }
    };
    // ── Append User Message Row ────────────────────────────────────────────────
    const appendUserMessage = (text) => {
        if (!messagesContainer)
            return;
        // Switch view from empty state to active message stream
        chatEmptyState?.classList.add('hidden');
        messagesContainer.classList.remove('hidden');
        let currentText = text;
        const row = document.createElement('div');
        row.className = 'user-message-row';
        row.innerHTML = `
      <div class="user-message-bubble">${escapeHtml(currentText)}</div>
      <div class="user-message-actions">
        <button class="user-action-btn btn-edit-user" title="Edit prompt">
          <span class="ui-icon icon-msg-edit"></span>
        </button>
        <button class="user-action-btn btn-copy-user" title="Copy text">
          <span class="ui-icon icon-msg-copy"></span>
        </button>
      </div>
    `;
        const bubble = row.querySelector('.user-message-bubble');
        const actions = row.querySelector('.user-message-actions');
        const copyBtn = row.querySelector('.btn-copy-user');
        const copyIcon = copyBtn?.querySelector('.ui-icon') || null;
        const editBtn = row.querySelector('.btn-edit-user');
        // Hook copy action
        copyBtn?.addEventListener('click', () => {
            copyWithFeedback(currentText, copyIcon, 'icon-msg-check', 'icon-msg-copy');
        });
        // Hook inline edit action
        editBtn?.addEventListener('click', () => {
            if (row.querySelector('.user-edit-container'))
                return;
            // Close any other open edit box
            document.querySelectorAll('.user-edit-container').forEach((el) => {
                const parentRow = el.closest('.user-message-row');
                if (parentRow) {
                    const b = parentRow.querySelector('.user-message-bubble');
                    const a = parentRow.querySelector('.user-message-actions');
                    if (b)
                        b.style.display = '';
                    if (a)
                        a.style.display = '';
                    el.remove();
                }
            });
            bubble.style.display = 'none';
            actions.style.display = 'none';
            const editContainer = document.createElement('div');
            editContainer.className = 'user-edit-container';
            editContainer.innerHTML = `
        <textarea class="user-edit-textarea" rows="1" placeholder="Edit prompt..."></textarea>
        <div class="user-edit-actions">
          <button class="user-edit-btn btn-user-edit-cancel">Cancel</button>
          <button class="user-edit-btn btn-user-edit-save">Save</button>
        </div>
      `;
            const textarea = editContainer.querySelector('.user-edit-textarea');
            const cancelBtn = editContainer.querySelector('.btn-user-edit-cancel');
            const saveBtn = editContainer.querySelector('.btn-user-edit-save');
            textarea.value = currentText;
            const autoResize = () => {
                textarea.style.height = 'auto';
                textarea.style.height = `${Math.min(300, Math.max(38, textarea.scrollHeight))}px`;
            };
            textarea.addEventListener('input', autoResize);
            setTimeout(autoResize, 0);
            const cancelEdit = () => {
                editContainer.remove();
                bubble.style.display = '';
                actions.style.display = '';
            };
            cancelBtn.addEventListener('click', cancelEdit);
            saveBtn.addEventListener('click', () => {
                const newText = textarea.value.trim();
                if (newText) {
                    currentText = newText;
                    bubble.innerText = newText;
                }
                cancelEdit();
            });
            row.appendChild(editContainer);
            textarea.focus();
        });
        messagesContainer.appendChild(row);
        scrollToBottom();
    };
    // ── Append Dummy Assistant Message Row ─────────────────────────────────────
    const appendAssistantMessage = (userPrompt) => {
        if (!messagesContainer)
            return;
        const responseText = `I received your prompt: "${userPrompt}"\n\nThis is a dummy response confirming that the user message bubble and assistant message stream are working seamlessly with the Operon design system.`;
        const row = document.createElement('div');
        row.className = 'assistant-message-row';
        row.innerHTML = `
      <div class="assistant-message-body">${escapeHtml(responseText).replace(/\n\n/g, '<br><br>')}</div>
      <div class="assistant-controls-container">
        <div class="assistant-separator-line"></div>
        <div class="assistant-action-bar">
          <img class="assistant-brand-logo" src="assets/brand/operon.svg" alt="Operon" />
          <button class="assistant-action-btn btn-asst-copy" title="Copy response">
            <span class="ui-icon icon-asst-copy"></span>
          </button>
          <button class="assistant-action-btn btn-asst-like" title="Good response">
            <span class="ui-icon icon-asst-like"></span>
          </button>
          <button class="assistant-action-btn btn-asst-dislike" title="Bad response">
            <span class="ui-icon icon-asst-dislike"></span>
          </button>
          <button class="assistant-action-btn btn-asst-fork" title="Fork conversation">
            <span class="ui-icon icon-asst-fork"></span>
          </button>
          <span class="assistant-time-text">Just now</span>
        </div>
      </div>
    `;
        // Hook copy action
        const copyBtn = row.querySelector('.btn-asst-copy');
        const copyIcon = copyBtn?.querySelector('.ui-icon') || null;
        copyBtn?.addEventListener('click', () => {
            copyWithFeedback(responseText, copyIcon, 'icon-asst-check', 'icon-asst-copy');
        });
        // Hook like/dislike toggle
        const likeBtn = row.querySelector('.btn-asst-like');
        const dislikeBtn = row.querySelector('.btn-asst-dislike');
        const controlsContainer = row.querySelector('.assistant-controls-container');
        likeBtn?.addEventListener('click', () => {
            const active = likeBtn.classList.toggle('active');
            dislikeBtn?.classList.remove('active');
            controlsContainer?.classList.toggle('has-active', active);
        });
        dislikeBtn?.addEventListener('click', () => {
            const active = dislikeBtn.classList.toggle('active');
            likeBtn?.classList.remove('active');
            controlsContainer?.classList.toggle('has-active', active);
        });
        messagesContainer.appendChild(row);
        scrollToBottom();
    };
    // ── Handle Submit Prompt ───────────────────────────────────────────────────
    const handleSubmit = () => {
        if (!inputPrompt)
            return;
        const text = inputPrompt.value.trim();
        if (!text)
            return;
        // Reset textarea
        inputPrompt.value = '';
        inputPrompt.style.height = 'auto';
        btnSend?.classList.add('disabled');
        // 1. Add user bubble immediately
        appendUserMessage(text);
        // 2. Add dummy assistant response after a short realistic delay
        setTimeout(() => {
            appendAssistantMessage(text);
        }, 350);
    };
    // ── Input Card Auto-Resize & Actions ──────────────────────────────────────
    if (inputPrompt && btnSend) {
        inputPrompt.addEventListener('input', () => {
            inputPrompt.style.height = 'auto';
            inputPrompt.style.height = `${Math.min(inputPrompt.scrollHeight, 180)}px`;
            const hasText = inputPrompt.value.trim().length > 0;
            btnSend.classList.toggle('disabled', !hasText);
        });
        // Enter to submit (Shift+Enter for newline)
        inputPrompt.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSubmit();
            }
        });
        btnSend.addEventListener('click', handleSubmit);
    }
    // ── Auto-Approve Toggle Pill ───────────────────────────────────────────────
    btnAutoApprove?.addEventListener('click', () => {
        const isNowActive = btnAutoApprove.classList.toggle('active');
        const icon = btnAutoApprove.querySelector('.ui-icon');
        if (icon) {
            icon.className = isNowActive
                ? 'ui-icon icon-input-auto-approve-enable'
                : 'ui-icon icon-input-auto-approve-disable';
        }
    });
    console.log('[Operon Webview] UI layout initialized successfully.');
});
//# sourceMappingURL=main.js.map