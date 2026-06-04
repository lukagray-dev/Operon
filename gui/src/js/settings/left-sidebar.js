'use strict';

/**
 * left-sidebar.js
 *
 * Builds and manages the left navigation sidebar inside the settings dialog.
 *
 * Public API:
 *  - buildLeftSidebar(categories, activeKey) → string   (HTML)
 *  - highlightNavItem(sidebarEl, key)                   (DOM mutation)
 *
 * Each category gets a .settings-nav__item button with a data-settings-category
 * attribute. Clicks are delegated in settings-panel.js, not here.
 *
 * Icons are loaded from: Operon/gui/src/assets/icons/settings/<filename>
 * They are inlined as <img> tags so no fetch is needed. The icons directory
 * already has: brain.svg, circle-user.svg, cog.svg, database.svg, palette.svg,
 * plug.svg, puzzle.svg, settings-2.svg, settings.svg, shield.svg, user.svg, x.svg
 */

// ── Path prefix for settings icons ───────────────────────────────────────────

/**
 * Relative URL path to the settings icon folder.
 * This is relative to the index.html served root (gui/src/index.html).
 * @type {string}
 */
const ICON_BASE = './assets/icons/settings/';

// ── Builder ───────────────────────────────────────────────────────────────────

/**
 * Builds the complete left sidebar HTML.
 *
 * @param {Array<{key: string, label: string, icon: string}>} categories
 *   Ordered list of navigation categories from CATEGORIES in settings-panel.js.
 * @param {string} activeKey
 *   The currently selected category key.
 * @returns {string} HTML string for the entire sidebar nav.
 */
function buildLeftSidebar(categories, activeKey) {
  // Build the list of nav item buttons
  const itemsHtml = categories.map(cat => buildNavItem(cat, activeKey)).join('');

  return `
    <div class="settings-nav">
      ${itemsHtml}
    </div>
  `;
}

// ── Nav Item Builder ──────────────────────────────────────────────────────────

/**
 * Builds a single navigation item button.
 *
 * @param {{key: string, label: string, icon: string}} category
 * @param {string} activeKey - Currently active category key.
 * @returns {string} HTML for one nav item.
 */
function buildNavItem(category, activeKey) {
  // Mark the active item so CSS can style it differently
  const isActive = category.key === activeKey;
  const activeClass = isActive ? ' is-active' : '';

  // Icon image — use an <img> tag pointing to the local assets
  const iconHtml = `
    <span class="settings-nav__item-icon" aria-hidden="true">
      <img src="${ICON_BASE}${category.icon}"
           alt=""
           width="16"
           height="16"
           draggable="false">
    </span>
  `;

  return `
    <button
      class="settings-nav__item${activeClass}"
      type="button"
      data-settings-category="${category.key}"
      aria-label="Go to ${category.label} settings"
      aria-current="${isActive ? 'page' : 'false'}">
      ${iconHtml}
      <span class="settings-nav__item-label">${category.label}</span>
    </button>
  `;
}

// ── Highlight Updater ─────────────────────────────────────────────────────────

/**
 * Updates the active highlight on nav items without re-rendering the sidebar.
 * Called by settings-panel.js whenever the category changes.
 *
 * @param {HTMLElement} sidebarEl - The sidebar nav container element.
 * @param {string}      key       - The newly active category key.
 */
function highlightNavItem(sidebarEl, key) {
  // Query all nav item buttons inside this sidebar
  const items = sidebarEl.querySelectorAll('[data-settings-category]');

  items.forEach(item => {
    const isActive = item.getAttribute('data-settings-category') === key;
    // Toggle the is-active class for visual highlight
    item.classList.toggle('is-active', isActive);
    // Update aria-current for screen readers
    item.setAttribute('aria-current', isActive ? 'page' : 'false');
  });
}

// ── Exports ───────────────────────────────────────────────────────────────────

export { buildLeftSidebar, highlightNavItem };
