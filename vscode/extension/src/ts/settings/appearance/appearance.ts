// Appearance Settings Panel Coordinator
// Manages Themes, Code/Table Appearance, Thinking Orbs, and Typography

import { ThinkingOrbRenderer } from '../../thinking-orbs/orb-renderer.js';
import { getAppearanceSettingsIpc, saveAppearanceSettingsIpc } from './ipc.js';
import type { AppearanceSettings } from './types.js';

let currentSettings: AppearanceSettings = {
  selected_theme: 0,
  selected_ui_scale: 1,
  compact_mode: false,
  smooth_animations: true,
  selected_thinking_orb: 0,
  selected_ui_font: 0,
  selected_assistant_font: 0,
  selected_code_font: 0,
  code_block_theme: 0,
  show_line_numbers: true,
  highlight_inline_code: true,
  table_theme: 0,
  orb_speed: 1,
  show_live_orb: true,
};

let orbRenderers: ThinkingOrbRenderer[] = [];

/**
 * Initializes the Appearance Settings panel and live previews.
 */
export async function initAppearanceSettings(): Promise<void> {
  try {
    currentSettings = await getAppearanceSettingsIpc();
  } catch (err) {
    console.warn('[AppearanceSettings] Failed to load settings:', err);
  }

  setupCodeThemeCards();
  setupTableThemeCards();
  setupToggleSwitches();
  setupOrbCards();
  setupOrbSpeedSelector();
  setupFontSelectors();
  startLiveOrbPreviews();
  syncAppearanceUI();
}

/**
 * Binds Code Block theme selection cards.
 */
function setupCodeThemeCards(): void {
  const cards = document.querySelectorAll<HTMLElement>('.theme-preview-card[data-type="code"]');
  cards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.code_block_theme = idx;
      updateCodeThemeCardsUI();
      await persist();
    });
  });
}

function updateCodeThemeCardsUI(): void {
  const cards = document.querySelectorAll<HTMLElement>('.theme-preview-card[data-type="code"]');
  cards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.code_block_theme);
  });
}

/**
 * Binds Table style selection cards.
 */
function setupTableThemeCards(): void {
  const cards = document.querySelectorAll<HTMLElement>('.theme-preview-card[data-type="table"]');
  cards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.table_theme = idx;
      updateTableThemeCardsUI();
      await persist();
    });
  });
}

function updateTableThemeCardsUI(): void {
  const cards = document.querySelectorAll<HTMLElement>('.theme-preview-card[data-type="table"]');
  cards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.table_theme);
  });
}

/**
 * Binds toggle switches.
 */
function setupToggleSwitches(): void {
  bindSwitch('toggle-app-line-numbers', currentSettings.show_line_numbers, async (val) => {
    currentSettings.show_line_numbers = val;
    await persist();
  });

  bindSwitch('toggle-app-inline-code', currentSettings.highlight_inline_code, async (val) => {
    currentSettings.highlight_inline_code = val;
    await persist();
  });

  bindSwitch('toggle-app-live-orb', currentSettings.show_live_orb, async (val) => {
    currentSettings.show_live_orb = val;
    await persist();
  });
}

/**
 * Binds the 4 Thinking & Reasoning Orb preview cards.
 */
function setupOrbCards(): void {
  const cards = document.querySelectorAll<HTMLElement>('.orb-selection-card');
  cards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.selected_thinking_orb = idx;
      updateOrbCardsUI();
      await persist();
    });
  });
}

function updateOrbCardsUI(): void {
  const cards = document.querySelectorAll<HTMLElement>('.orb-selection-card');
  cards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.selected_thinking_orb);
  });
}

/**
 * Binds Orb Animation Speed selector (1.5x, 3.0x, 4.5x).
 */
function setupOrbSpeedSelector(): void {
  const buttons = document.querySelectorAll<HTMLButtonElement>('.seg-choice-orb-speed');
  buttons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      const idx = parseInt(btn.dataset.index || '1', 10);
      currentSettings.orb_speed = idx;
      updateOrbSpeedUI();
      const mult = getSpeedMultiplier(idx);
      orbRenderers.forEach((r) => r.setSpeed(mult));
      await persist();
    });
  });
}

function updateOrbSpeedUI(): void {
  const buttons = document.querySelectorAll<HTMLButtonElement>('.seg-choice-orb-speed');
  buttons.forEach((btn) => {
    const idx = parseInt(btn.dataset.index || '1', 10);
    btn.classList.toggle('active', idx === currentSettings.orb_speed);
  });
}

function getSpeedMultiplier(idx: number): number {
  switch (idx) {
    case 0:
      return 1.5;
    case 2:
      return 4.5;
    default:
      return 3.0;
  }
}

/**
 * Initializes the 4 live thinking orb preview canvases with the thinking-orbs engine.
 */
function startLiveOrbPreviews(): void {
  // Clean up any existing instances
  orbRenderers.forEach((r) => r.destroy());
  orbRenderers = [];

  const speed = getSpeedMultiplier(currentSettings.orb_speed);

  const configs: Array<{ id: string; state: 'composing' | 'shaping' | 'working' | 'connecting' }> = [
    { id: 'canvas-orb-composing', state: 'composing' },
    { id: 'canvas-orb-shaping', state: 'shaping' },
    { id: 'canvas-orb-working', state: 'working' },
    { id: 'canvas-orb-connecting', state: 'connecting' },
  ];

  configs.forEach(({ id, state }) => {
    const canvas = document.getElementById(id) as HTMLCanvasElement | null;
    if (canvas) {
      try {
        const renderer = new ThinkingOrbRenderer(canvas, {
          state,
          size: 64,
          speed,
          dark: true,
        });
        renderer.start();
        orbRenderers.push(renderer);
      } catch (err) {
        console.warn(`[AppearanceSettings] Failed to create ThinkingOrbRenderer for ${id}:`, err);
      }
    }
  });
}

/**
 * Binds typography font preview cards (UI Font, Assistant Font, Code Font).
 */
function setupFontSelectors(): void {
  // UI Font Cards
  const uiCards = document.querySelectorAll<HTMLElement>('.font-preview-card[data-type="ui-font"]');
  uiCards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.selected_ui_font = idx;
      updateFontCardsUI();
      applyLiveFonts();
      await persist();
    });
  });

  // Assistant Font Cards
  const astCards = document.querySelectorAll<HTMLElement>('.font-preview-card[data-type="assistant-font"]');
  astCards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.selected_assistant_font = idx;
      updateFontCardsUI();
      applyLiveFonts();
      await persist();
    });
  });

  // Code Font Cards
  const codeCards = document.querySelectorAll<HTMLElement>('.font-preview-card[data-type="code-font"]');
  codeCards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.selected_code_font = idx;
      updateFontCardsUI();
      applyLiveFonts();
      await persist();
    });
  });
}

function updateFontCardsUI(): void {
  const uiCards = document.querySelectorAll<HTMLElement>('.font-preview-card[data-type="ui-font"]');
  uiCards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.selected_ui_font);
  });

  const astCards = document.querySelectorAll<HTMLElement>('.font-preview-card[data-type="assistant-font"]');
  astCards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.selected_assistant_font);
  });

  const codeCards = document.querySelectorAll<HTMLElement>('.font-preview-card[data-type="code-font"]');
  codeCards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.selected_code_font);
  });
}

function applyLiveFonts(): void {
  const root = document.documentElement;

  // UI Font
  const uiFonts = [
    "'Open Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    "'Roboto', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  ];
  root.style.setProperty('--font-family', uiFonts[currentSettings.selected_ui_font] || uiFonts[0]);

  // Assistant Font
  const astFonts = [
    "'Literata', Georgia, serif",
    "'Lora', Georgia, serif",
    "'Merriweather', Georgia, serif",
  ];
  root.style.setProperty('--assistant-font-family', astFonts[currentSettings.selected_assistant_font] || astFonts[0]);

  // Code Font
  const codeFonts = [
    "'Kode Mono', monospace",
    "'JetBrains Mono', monospace",
    "'Fira Code', monospace",
  ];
  root.style.setProperty('--mono-font-family', codeFonts[currentSettings.selected_code_font] || codeFonts[0]);
}

function bindSwitch(id: string, initial: boolean, onToggle: (checked: boolean) => Promise<void>): void {
  const switchEl = document.getElementById(id);
  if (!switchEl) return;

  switchEl.classList.toggle('checked', initial);
  switchEl.setAttribute('aria-checked', String(initial));

  switchEl.addEventListener('click', async () => {
    const isChecked = switchEl.classList.toggle('checked');
    switchEl.setAttribute('aria-checked', String(isChecked));
    await onToggle(isChecked);
  });
}

function syncAppearanceUI(): void {
  updateCodeThemeCardsUI();
  updateTableThemeCardsUI();
  updateOrbCardsUI();
  updateOrbSpeedUI();
  updateFontCardsUI();

  setSwitchChecked('toggle-app-line-numbers', currentSettings.show_line_numbers);
  setSwitchChecked('toggle-app-inline-code', currentSettings.highlight_inline_code);
  setSwitchChecked('toggle-app-live-orb', currentSettings.show_live_orb);

  applyLiveFonts();
}

function setSwitchChecked(id: string, checked: boolean): void {
  const el = document.getElementById(id);
  if (el) {
    el.classList.toggle('checked', checked);
    el.setAttribute('aria-checked', String(checked));
  }
}

async function persist(): Promise<void> {
  try {
    await saveAppearanceSettingsIpc(currentSettings);
  } catch (err) {
    console.error('[AppearanceSettings] Persist failed:', err);
  }
}
