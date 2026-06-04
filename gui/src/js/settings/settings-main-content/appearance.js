'use strict';

/**
 * appearance.js
 *
 * Appearance settings page for the Operon settings panel.
 *
 * Provides:
 *  - Theme selection (System / Dark / Light) via preview cards
 *  - Accent color presets + custom color picker
 *  - Chat and code font size sliders
 *  - UI and code font-family dropdowns
 *  - Chat content width (segmented control)
 *  - Code line numbers and reduced motion accessibility toggles
 *
 * All changes are applied immediately via CSS custom properties on <html>
 * and persisted to the store.
 *
 * applyAppearanceSettings() is called at app startup from settings-panel.js
 * to restore the user's saved preferences before the first render.
 */
import { showSuccess } from '../../shared/toast.js';

// ── Local Settings State (Placeholder UI) ────────────────────────────────────

/**
 * In-memory settings for the placeholder UI.
 * In production, this would be persisted to disk/backend.
 */
const localSettings = {
  theme: 'dark',
  accentColor: '#0075e3',
  chatFontSize: 14,
  codeFontSize: 13,
  uiFont: 'default',
  codeFont: 'default',
  chatWidth: 'standard',
  codeLineNumbers: true,
  reducedMotion: false,
};

// ── Constants ────────────────────────────────────────────────────────────────

/**
 * Preset accent color swatches displayed in the grid.
 * @type {Array<{key: string, hex: string, label: string}>}
 */
const ACCENT_PRESETS = [
  { key: 'blue',   hex: '#0075e3', label: 'Blue'   },
  { key: 'indigo', hex: '#6366F1', label: 'Indigo' },
  { key: 'purple', hex: '#8B5CF6', label: 'Purple' },
  { key: 'pink',   hex: '#EC4899', label: 'Pink'   },
  { key: 'red',    hex: '#EF4444', label: 'Red'    },
  { key: 'orange', hex: '#F59E0B', label: 'Orange' },
  { key: 'green',  hex: '#22C55E', label: 'Green'  },
  { key: 'teal',   hex: '#14B8A6', label: 'Teal'   },
  { key: 'cyan',   hex: '#06B6D4', label: 'Cyan'   },
];

/**
 * Available UI (interface) font families.
 * @type {Array<{key: string, label: string, value: string}>}
 */
const UI_FONTS = [
  { key: 'default', label: 'Google Sans', value: "'Google Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif" },
  { key: 'system',  label: 'System UI',   value: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif"        },
  { key: 'inter',   label: 'Inter',       value: "'Inter', -apple-system, BlinkMacSystemFont, sans-serif"                   },
  { key: 'roboto',  label: 'Roboto',      value: "'Roboto', -apple-system, BlinkMacSystemFont, sans-serif"                  },
  { key: 'serif',   label: 'PT Serif',    value: "'PT Serif', Georgia, 'Times New Roman', serif"                            },
];

/**
 * Available code font families.
 * @type {Array<{key: string, label: string, value: string}>}
 */
const CODE_FONTS = [
  { key: 'default',   label: 'Kode Mono',      value: "'Kode Mono', 'JetBrains Mono', monospace"    },
  { key: 'jetbrains', label: 'JetBrains Mono', value: "'JetBrains Mono', 'Fira Code', monospace"    },
  { key: 'firacode',  label: 'Fira Code',      value: "'Fira Code', 'JetBrains Mono', monospace"    },
  { key: 'cascadia',  label: 'Cascadia Code',  value: "'Cascadia Code', 'Consolas', monospace"       },
  { key: 'consolas',  label: 'Consolas',       value: "'Consolas', 'Courier New', monospace"         },
];

/**
 * Chat content width presets.
 * @type {Array<{key: string, label: string, value: string}>}
 */
const CHAT_WIDTHS = [
  { key: 'narrow',   label: 'Narrow',   value: '640px'  },
  { key: 'standard', label: 'Standard', value: '768px'  },
  { key: 'wide',     label: 'Wide',     value: '960px'  },
  { key: 'full',     label: 'Full',     value: '100%'   },
];

// Font size bounds
const CHAT_FONT_MIN     = 12;
const CHAT_FONT_MAX     = 20;
const CHAT_FONT_DEFAULT = 14;
const CODE_FONT_MIN     = 10;
const CODE_FONT_MAX     = 18;
const CODE_FONT_DEFAULT = 13;

// ── Color Utility ─────────────────────────────────────────────────────────────

/**
 * Adjusts the brightness of a hex color.
 * Positive percent = lighter, negative = darker.
 *
 * @param {string} hex      - 7-char hex string like '#0075e3'.
 * @param {number} percent  - Amount to shift, e.g. -25 darkens by 25%.
 * @returns {string} Adjusted hex color string.
 */
function adjustBrightness(hex, percent) {
  // Parse R, G, B channels from the hex string
  let r = parseInt(hex.slice(1, 3), 16);
  let g = parseInt(hex.slice(3, 5), 16);
  let b = parseInt(hex.slice(5, 7), 16);

  // Scale each channel and clamp to valid byte range [0, 255]
  const factor = 1 + percent / 100;
  r = Math.max(0, Math.min(255, Math.round(r * factor)));
  g = Math.max(0, Math.min(255, Math.round(g * factor)));
  b = Math.max(0, Math.min(255, Math.round(b * factor)));

  // Re-compose the hex string with zero-padding
  return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
}

// ── Page Builder ──────────────────────────────────────────────────────────────

/**
 * Builds the full HTML for the Appearance settings page.
 *
 * @param {Object} settings - User settings from the store.
 * @param {Object} state    - Full app state.
 * @returns {string} HTML string.
 */
function buildAppearancePage(settings) {
  // Pull current values from local settings with safe defaults
  const currentTheme    = localSettings.theme || 'dark';
  const accentColor     = localSettings.accentColor  || '#0075e3';
  const chatFontSize    = localSettings.chatFontSize  ?? CHAT_FONT_DEFAULT;
  const codeFontSize    = localSettings.codeFontSize  ?? CODE_FONT_DEFAULT;
  const uiFont          = localSettings.uiFont        || 'default';
  const codeFont        = localSettings.codeFont      || 'default';
  const chatWidth       = localSettings.chatWidth     || 'standard';
  const codeLineNumbers = localSettings.codeLineNumbers !== false;  // default on
  const reducedMotion   = localSettings.reducedMotion === true;     // default off

  return `
    <div class="settings-page settings-appearance">
      <h2 class="settings-page__title">Appearance</h2>
      <p class="settings-page__description">Customize the look and feel of Operon</p>

      <!-- ── Theme ─────────────────────────────────────────────────────── -->
      <div class="settings-section">
        <div class="settings-section__header">
          <h3 class="settings-section__title">Theme</h3>
          <p class="settings-section__description">Choose your preferred color scheme</p>
        </div>
        <div class="appearance-theme-cards" id="appearance-theme-cards">
          ${buildThemeCard('system', 'System', 'Follows your OS setting', currentTheme)}
          ${buildThemeCard('dark',   'Dark',   'Easy on the eyes',        currentTheme)}
          ${buildThemeCard('light',  'Light',  'Classic bright look',     currentTheme)}
        </div>
      </div>

      <!-- ── Accent Color ───────────────────────────────────────────────── -->
      <div class="settings-section">
        <div class="settings-section__header">
          <h3 class="settings-section__title">Accent color</h3>
          <p class="settings-section__description">Primary highlight color used across the interface</p>
        </div>
        <div class="appearance-accent-grid" id="appearance-accent-grid">
          ${ACCENT_PRESETS.map(p => `
            <button class="appearance-accent-swatch ${p.hex.toLowerCase() === accentColor.toLowerCase() ? 'is-selected' : ''}"
                    data-accent="${p.hex}"
                    title="${p.label}"
                    aria-label="Accent color: ${p.label}"
                    style="--swatch-color: ${p.hex}">
              <span class="appearance-accent-swatch__check">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24"
                     fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                  <polyline points="20 6 9 17 4 12"/>
                </svg>
              </span>
            </button>
          `).join('')}
          <!-- Custom color picker input -->
          <label class="appearance-accent-custom" title="Custom color">
            <input type="color" id="appearance-accent-custom-input"
                   value="${accentColor}"
                   class="appearance-accent-custom__input">
            <span class="appearance-accent-custom__label">Custom</span>
          </label>
        </div>
      </div>

      <!-- ── Typography ────────────────────────────────────────────────── -->
      <div class="settings-section">
        <div class="settings-section__header">
          <h3 class="settings-section__title">Typography</h3>
          <p class="settings-section__description">Font sizes and families for the interface and code</p>
        </div>

        <!-- Chat font size slider -->
        <div class="appearance-slider-row">
          <div class="appearance-slider-row__info">
            <span class="appearance-slider-row__label">Chat font size</span>
            <span class="appearance-slider-row__value" id="appearance-chat-font-value">${chatFontSize}px</span>
          </div>
          <input type="range" class="appearance-range" id="appearance-chat-font-slider"
                 min="${CHAT_FONT_MIN}" max="${CHAT_FONT_MAX}" step="1" value="${chatFontSize}">
        </div>

        <!-- Code font size slider -->
        <div class="appearance-slider-row">
          <div class="appearance-slider-row__info">
            <span class="appearance-slider-row__label">Code font size</span>
            <span class="appearance-slider-row__value" id="appearance-code-font-value">${codeFontSize}px</span>
          </div>
          <input type="range" class="appearance-range" id="appearance-code-font-slider"
                 min="${CODE_FONT_MIN}" max="${CODE_FONT_MAX}" step="1" value="${codeFontSize}">
        </div>

        <!-- UI font family dropdown -->
        <div class="appearance-slider-row">
          <div class="appearance-slider-row__info">
            <span class="appearance-slider-row__label">UI font</span>
          </div>
          <select class="setting-select" id="appearance-ui-font">
            ${UI_FONTS.map(f => `<option value="${f.key}" ${f.key === uiFont ? 'selected' : ''}>${f.label}</option>`).join('')}
          </select>
        </div>

        <!-- Code font family dropdown -->
        <div class="appearance-slider-row">
          <div class="appearance-slider-row__info">
            <span class="appearance-slider-row__label">Code font</span>
          </div>
          <select class="setting-select" id="appearance-code-font">
            ${CODE_FONTS.map(f => `<option value="${f.key}" ${f.key === codeFont ? 'selected' : ''}>${f.label}</option>`).join('')}
          </select>
        </div>
      </div>

      <!-- ── Layout ────────────────────────────────────────────────────── -->
      <div class="settings-section">
        <div class="settings-section__header">
          <h3 class="settings-section__title">Layout</h3>
          <p class="settings-section__description">Adjust the chat content width</p>
        </div>
        <div class="appearance-segmented" id="appearance-chat-width">
          ${CHAT_WIDTHS.map(w => `
            <button class="appearance-segmented__btn ${w.key === chatWidth ? 'is-active' : ''}"
                    data-width="${w.key}">${w.label}</button>
          `).join('')}
        </div>
      </div>

      <!-- ── Accessibility ──────────────────────────────────────────────── -->
      <div class="settings-section">
        <div class="settings-section__header">
          <h3 class="settings-section__title">Accessibility</h3>
        </div>

        <!-- Code line numbers toggle -->
        <div class="appearance-toggle-row">
          <div class="appearance-toggle-row__info">
            <span class="appearance-toggle-row__label">Code line numbers</span>
            <span class="appearance-toggle-row__desc">Show line numbers inside code blocks</span>
          </div>
          <label class="setting-toggle">
            <input type="checkbox" id="appearance-code-line-numbers" ${codeLineNumbers ? 'checked' : ''}>
            <span class="setting-toggle__slider"></span>
          </label>
        </div>

        <!-- Reduced motion toggle -->
        <div class="appearance-toggle-row">
          <div class="appearance-toggle-row__info">
            <span class="appearance-toggle-row__label">Reduced motion</span>
            <span class="appearance-toggle-row__desc">Minimize animations across the app</span>
          </div>
          <label class="setting-toggle">
            <input type="checkbox" id="appearance-reduced-motion" ${reducedMotion ? 'checked' : ''}>
            <span class="setting-toggle__slider"></span>
          </label>
        </div>
      </div>
    </div>
  `;
}

// ── Theme Card Builder ────────────────────────────────────────────────────────

/**
 * Builds a single theme preview card.
 *
 * @param {string} value   - 'system' | 'dark' | 'light'
 * @param {string} label   - Display name.
 * @param {string} desc    - Short description.
 * @param {string} current - Currently active theme value.
 * @returns {string} HTML.
 */
function buildThemeCard(value, label, desc, current) {
  const isSelected = value === current;
  return `
    <button class="appearance-theme-card ${isSelected ? 'is-selected' : ''}"
            data-theme-value="${value}"
            aria-pressed="${isSelected}"
            type="button">
      <div class="appearance-theme-card__preview appearance-theme-card__preview--${value}">
        <div class="appearance-theme-card__sidebar-preview"></div>
        <div class="appearance-theme-card__content-preview">
          <div class="appearance-theme-card__line"></div>
          <div class="appearance-theme-card__line appearance-theme-card__line--short"></div>
          <div class="appearance-theme-card__line appearance-theme-card__line--medium"></div>
        </div>
      </div>
      <div class="appearance-theme-card__info">
        <span class="appearance-theme-card__label">${label}</span>
        <span class="appearance-theme-card__desc">${desc}</span>
      </div>
    </button>
  `;
}

// ── Hydration ────────────────────────────────────────────────────────────────

/**
 * Attaches all event listeners to the appearance page controls.
 * Must be called after the page HTML is inserted into the DOM.
 *
 * @param {HTMLElement} container - The settings main content element.
 */
function hydrateAppearancePage(container) {
  if (!container) return;

  // ── Theme cards ──
  const themeCards = container.querySelectorAll('.appearance-theme-card');
  themeCards.forEach(card => {
    card.addEventListener('click', () => {
      const value = card.dataset.themeValue;
      if (!value) return;

      // Toggle visual selection state
      themeCards.forEach(c => {
        c.classList.toggle('is-selected', c === card);
        c.setAttribute('aria-pressed', String(c === card));
      });

      // Persist and apply the new theme
      localSettings.theme = value;
      applyThemeValue(value);
      showSuccess(`Theme set to ${value}`);
    });
  });

  // ── Accent color swatches ──
  const swatches = container.querySelectorAll('.appearance-accent-swatch');
  const customInput = /** @type {HTMLInputElement|null} */ (container.querySelector('#appearance-accent-custom-input'));

  swatches.forEach(swatch => {
    swatch.addEventListener('click', () => {
      const hex = swatch.dataset.accent;
      if (!hex) return;

      // Update selection indicators
      swatches.forEach(s => s.classList.toggle('is-selected', s === swatch));
      if (customInput) customInput.value = hex;

      // Persist and apply immediately
      localSettings.accentColor = hex;
      applyAccentColor(hex);
    });
  });

  // Custom color picker fires on every color change
  if (customInput) {
    customInput.addEventListener('input', () => {
      const hex = customInput.value;
      swatches.forEach(s => s.classList.remove('is-selected'));
      localSettings.accentColor = hex;
      applyAccentColor(hex);
    });
  }

  // ── Chat font size slider ──
  const chatSlider = /** @type {HTMLInputElement|null} */ (container.querySelector('#appearance-chat-font-slider'));
  const chatValue  = container.querySelector('#appearance-chat-font-value');
  if (chatSlider) {
    chatSlider.addEventListener('input', () => {
      const size = parseInt(chatSlider.value, 10);
      if (chatValue) chatValue.textContent = `${size}px`;
      applyChatFontSize(size);
      localSettings.chatFontSize = size;
    });
  }

  // ── Code font size slider ──
  const codeSlider = /** @type {HTMLInputElement|null} */ (container.querySelector('#appearance-code-font-slider'));
  const codeValue  = container.querySelector('#appearance-code-font-value');
  if (codeSlider) {
    codeSlider.addEventListener('input', () => {
      const size = parseInt(codeSlider.value, 10);
      if (codeValue) codeValue.textContent = `${size}px`;
      applyCodeFontSize(size);
      localSettings.codeFontSize = size;
    });
  }

  // ── UI font dropdown ──
  const uiFontSelect = /** @type {HTMLSelectElement|null} */ (container.querySelector('#appearance-ui-font'));
  if (uiFontSelect) {
    uiFontSelect.addEventListener('change', () => {
      const key = uiFontSelect.value;
      localSettings.uiFont = key;
      applyUIFont(key);
    });
  }

  // ── Code font dropdown ──
  const codeFontSelect = /** @type {HTMLSelectElement|null} */ (container.querySelector('#appearance-code-font'));
  if (codeFontSelect) {
    codeFontSelect.addEventListener('change', () => {
      const key = codeFontSelect.value;
      localSettings.codeFont = key;
      applyCodeFont(key);
    });
  }

  // ── Chat width segmented control ──
  const widthBtns = container.querySelectorAll('#appearance-chat-width .appearance-segmented__btn');
  widthBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const key = btn.dataset.width;
      if (!key) return;
      widthBtns.forEach(b => b.classList.toggle('is-active', b === btn));
      localSettings.chatWidth = key;
      applyChatWidth(key);
    });
  });

  // ── Code line numbers toggle ──
  const lineNumToggle = /** @type {HTMLInputElement|null} */ (container.querySelector('#appearance-code-line-numbers'));
  if (lineNumToggle) {
    lineNumToggle.addEventListener('change', () => {
      localSettings.codeLineNumbers = lineNumToggle.checked;
      applyCodeLineNumbers(lineNumToggle.checked);
    });
  }

  // ── Reduced motion toggle ──
  const motionToggle = /** @type {HTMLInputElement|null} */ (container.querySelector('#appearance-reduced-motion'));
  if (motionToggle) {
    motionToggle.addEventListener('change', () => {
      localSettings.reducedMotion = motionToggle.checked;
      applyReducedMotion(motionToggle.checked);
    });
  }
}

// ── CSS Variable Application Helpers ─────────────────────────────────────────
// Each sets the corresponding CSS custom property on <html> so the change
// cascades to every element instantly without a page reload.

/** Applies the chosen theme by setting the data-theme attribute on <html>. */
function applyThemeValue(theme) {
  if (theme === 'system') {
    // Resolve to the OS preference immediately
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    document.documentElement.setAttribute('data-theme', prefersDark ? 'dark' : 'light');
  } else {
    document.documentElement.setAttribute('data-theme', theme);
  }
}

/** Sets accent-primary and derived hover / secondary CSS variables. */
function applyAccentColor(hex) {
  const root = document.documentElement;
  root.style.setProperty('--accent-primary',       hex);
  root.style.setProperty('--accent-primary-hover',  adjustBrightness(hex, -20));
  root.style.setProperty('--accent-secondary',      adjustBrightness(hex, 40));
}

/** Sets the chat message font size CSS variable. */
function applyChatFontSize(px) {
  document.documentElement.style.setProperty('--chat-font-size', `${px}px`);
}

/** Sets the code block font size CSS variable. */
function applyCodeFontSize(px) {
  document.documentElement.style.setProperty('--code-font-size', `${px}px`);
}

/** Sets the UI font-family CSS variable from the key. */
function applyUIFont(key) {
  const entry = UI_FONTS.find(f => f.key === key);
  if (entry) {
    document.documentElement.style.setProperty('--font-family-base', entry.value);
  }
}

/** Sets the code font-family CSS variable from the key. */
function applyCodeFont(key) {
  const entry = CODE_FONTS.find(f => f.key === key);
  if (entry) {
    document.documentElement.style.setProperty('--font-family-mono', entry.value);
  }
}

/** Sets the chat content max-width CSS variable from the key. */
function applyChatWidth(key) {
  const entry = CHAT_WIDTHS.find(w => w.key === key);
  if (entry) {
    document.documentElement.style.setProperty('--chat-max-width', entry.value);
  }
}

/** Toggles a data attribute on <html> to enable/disable code line numbers. */
function applyCodeLineNumbers(show) {
  document.documentElement.setAttribute('data-code-line-numbers', String(show));
}

/** Toggles a data attribute on <html> to enable/disable reduced motion. */
function applyReducedMotion(enabled) {
  document.documentElement.setAttribute('data-reduced-motion', String(enabled));
}

// ── Startup Applicator ────────────────────────────────────────────────────────

/**
 * Reads all persisted appearance settings from the store and applies them.
 * Call this ONCE at app startup — before the first render — so saved
 * preferences are reflected immediately.
 */
function applyAppearanceSettings() {
  const s = localSettings;

  // Theme
  applyThemeValue(s.theme || 'dark');

  // Accent color
  if (s.accentColor) applyAccentColor(s.accentColor);

  // Font sizes
  applyChatFontSize(s.chatFontSize ?? CHAT_FONT_DEFAULT);
  applyCodeFontSize(s.codeFontSize ?? CODE_FONT_DEFAULT);

  // Font families (only if non-default to avoid overriding CSS variables
  // that may already be set by the stylesheet)
  if (s.uiFont && s.uiFont !== 'default')     applyUIFont(s.uiFont);
  if (s.codeFont && s.codeFont !== 'default') applyCodeFont(s.codeFont);

  // Chat width
  if (s.chatWidth) applyChatWidth(s.chatWidth);

  // Accessibility
  applyCodeLineNumbers(s.codeLineNumbers !== false);
  applyReducedMotion(s.reducedMotion === true);

  // Listen for OS theme changes when "system" is selected
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (localSettings.theme === 'system') {
      applyThemeValue('system');
    }
  });
}

/** No transient state to reset — stub for panel lifecycle consistency. */
function resetAppearanceSettingsState() {
  // All state lives in the store — nothing transient to clean up.
}

// ── Exports ───────────────────────────────────────────────────────────────────

export {
  buildAppearancePage,
  hydrateAppearancePage,
  applyAppearanceSettings,
  resetAppearanceSettingsState,
  // Constants re-exported for tests
  ACCENT_PRESETS,
  UI_FONTS,
  CODE_FONTS,
  CHAT_WIDTHS,
  adjustBrightness,
};
