// ============================================================================
// Operon Webview Root Coordinator
//
// Hey friend! This is the main entrypoint running inside our Webview DOM.
// It initializes and coordinates the UI layout:
// 1. Left Sidebar (collapsible overlay drawer matching GUI structure)
// 2. Three-dots Conversation Context Menu Dropdown
// 3. Topbar with hamburger / sidebar toggle button
// 4. Central chat viewport with Operon empty state
// 5. Pinned bottom floating input card with auto-resizing textarea
// ============================================================================

window.addEventListener('DOMContentLoaded', () => {
  console.log('[Operon Webview] Initializing UI layout...');

  // DOM Elements
  const btnToggleSidebar = document.getElementById('btn-toggle-sidebar');
  const btnCloseSidebar = document.getElementById('btn-close-sidebar');
  const sidebarBackdrop = document.getElementById('sidebar-backdrop');
  const leftSidebar = document.getElementById('left-sidebar');
  const inputPrompt = document.getElementById('chat-input-textarea') as HTMLTextAreaElement | null;
  const btnSend = document.getElementById('btn-send-message');
  const btnAutoApprove = document.getElementById('btn-auto-approve');
  const searchInput = document.getElementById('sidebar-search-input') as HTMLInputElement | null;
  const btnSearchClear = document.getElementById('btn-search-clear');

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
    } else {
      openSidebar();
    }
  });

  // Close when clicking close (✕) button or dimmed backdrop
  btnCloseSidebar?.addEventListener('click', closeSidebar);
  sidebarBackdrop?.addEventListener('click', closeSidebar);

  // Close on Escape key
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (activeContextMenu) {
        dismissContextMenu();
      } else if (leftSidebar?.classList.contains('open')) {
        closeSidebar();
      }
    }
  });

  // Section Accordion Header Collapse / Expand
  document.querySelectorAll('.section-header').forEach((header) => {
    header.addEventListener('click', (e) => {
      // Don't toggle section if clicked on the action button (e.g. + folder)
      if ((e.target as HTMLElement).closest('.section-action-btn')) {
        return;
      }
      const section = header.closest('.sidebar-section');
      section?.classList.toggle('collapsed');
    });
  });

  // Project Header Collapse / Expand
  document.querySelectorAll('.project-header').forEach((header) => {
    header.addEventListener('click', (e) => {
      // Don't toggle if clicked on more button
      if ((e.target as HTMLElement).closest('.item-more-btn')) {
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
  let activeContextMenu: HTMLElement | null = null;

  function dismissContextMenu() {
    if (activeContextMenu) {
      activeContextMenu.remove();
      activeContextMenu = null;
    }
    document.querySelectorAll('.item-more-btn.active').forEach((btn) => {
      btn.classList.remove('active');
    });
  }

  function showContextMenu(e: MouseEvent, targetBtn: HTMLElement) {
    e.stopPropagation();
    dismissContextMenu();

    targetBtn.classList.add('active');
    const rect = targetBtn.getBoundingClientRect();

    const menu = document.createElement('div');
    menu.className = 'session-context-menu';

    // Position adjacent to the trigger button
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
  document.querySelectorAll<HTMLElement>('.item-more-btn').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      showContextMenu(e, btn);
    });
  });

  // Global dismiss on outside click
  window.addEventListener('click', (e) => {
    if (activeContextMenu && !(e.target as HTMLElement).closest('.session-context-menu')) {
      dismissContextMenu();
    }
  });

  // ── Input Card Auto-Resize & Actions ──────────────────────────────────────
  if (inputPrompt && btnSend) {
    inputPrompt.addEventListener('input', () => {
      // Auto expand textarea height up to 180px
      inputPrompt.style.height = 'auto';
      inputPrompt.style.height = `${Math.min(inputPrompt.scrollHeight, 180)}px`;

      // Enable/disable send button based on text
      const hasText = inputPrompt.value.trim().length > 0;
      btnSend.classList.toggle('disabled', !hasText);
    });

    // Enter to submit (Shift+Enter for newline)
    inputPrompt.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        if (inputPrompt.value.trim().length > 0) {
          inputPrompt.value = '';
          inputPrompt.style.height = 'auto';
          btnSend.classList.add('disabled');
        }
      }
    });

    btnSend.addEventListener('click', () => {
      if (inputPrompt.value.trim().length > 0) {
        inputPrompt.value = '';
        inputPrompt.style.height = 'auto';
        btnSend.classList.add('disabled');
      }
    });
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
