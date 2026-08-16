// Appearance Settings Controller & DOM Coordinator
//
// 1:1 implementation matching Slint appearance.slint:
// - Section 1: Theme & Color Palette (4 syntax code preview cards)
// - Section 2: UI Scale & Thinking Orb (5 scale levels, Compact mode, Smooth animations, 3 Live Orb cards)
// - Section 3: Typography & Code Styling (UI Font, Assistant Font, Code Font, Cursor Blink)

import { getAppearanceSettingsIpc, saveAppearanceSettingsIpc } from './ipc.js';
import type { AppearanceSettings } from './types.js';

let currentSettings: AppearanceSettings = {
  selected_theme: 0,
  selected_ui_scale: 1,
  compact_mode: false,
  smooth_animations: true,
  selected_thinking_orb: 1,
  selected_ui_font: 0,
  selected_assistant_font: 0,
  selected_code_font: 0,
  code_block_theme: 0,
  show_line_numbers: true,
  highlight_inline_code: true,
  table_theme: 0,
};

let orbAnimReq: number | null = null;

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
  setupScaleSelector();
  setupToggleSwitches();
  setupOrbCards();
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
 * Binds Table theme selection cards.
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
 * Binds UI Scaling preset buttons (80%, 100%, 120%, 140%, 160%).
 */
function setupScaleSelector(): void {
  const buttons = document.querySelectorAll<HTMLButtonElement>('.scale-option-btn');
  buttons.forEach((btn) => {
    btn.addEventListener('click', async () => {
      const idx = parseInt(btn.dataset.index || '1', 10);
      currentSettings.selected_ui_scale = idx;
      updateScaleUI();
      applyLiveScale(idx);
      await persist();
    });
  });
}

function updateScaleUI(): void {
  const buttons = document.querySelectorAll<HTMLButtonElement>('.scale-option-btn');
  buttons.forEach((btn) => {
    const idx = parseInt(btn.dataset.index || '1', 10);
    btn.classList.toggle('active', idx === currentSettings.selected_ui_scale);
  });
}

function applyLiveScale(scaleIndex: number): void {
  const scales = [0.8, 1.0, 1.2, 1.4, 1.6];
  const scale = scales[scaleIndex] || 1.0;
  document.documentElement.style.setProperty('--ui-scale', String(scale));
}

/**
 * Binds compact mode, smooth animations, and cursor blink toggles.
 */
function setupToggleSwitches(): void {
  // Section 1: Code Block Line Numbers & Inline Code
  bindSwitch('toggle-app-line-numbers', currentSettings.show_line_numbers, async (val) => {
    currentSettings.show_line_numbers = val;
    await persist();
  });

  bindSwitch('toggle-app-inline-code', currentSettings.highlight_inline_code, async (val) => {
    currentSettings.highlight_inline_code = val;
    await persist();
  });

  // Section 2: Compact mode and smooth animations
  bindSwitch('toggle-app-compact', currentSettings.compact_mode, async (val) => {
    currentSettings.compact_mode = val;
    await persist();
  });

  bindSwitch('toggle-app-animations', currentSettings.smooth_animations, async (val) => {
    currentSettings.smooth_animations = val;
    await persist();
  });
}

/**
 * Binds the 3 Thinking & Reasoning Orb preview cards.
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
 * Continuous 60fps canvas renderer for the 3 thinking orb preview canvases.
 */
function startLiveOrbPreviews(): void {
  if (orbAnimReq) cancelAnimationFrame(orbAnimReq);

  const canvas0 = document.getElementById('canvas-orb-breathing') as HTMLCanvasElement | null;
  const canvas1 = document.getElementById('canvas-orb-composing') as HTMLCanvasElement | null;
  const canvas2 = document.getElementById('canvas-orb-solving') as HTMLCanvasElement | null;

  let startT = performance.now();

  const drawFrame = (now: number) => {
    const elapsed = (now - startT) / 1000.0;

    // 0: Breathing Orb (Cosmic Ripple)
    if (canvas0) {
      drawBreathingOrb(canvas0, elapsed);
    }

    // 1: Composing Orb (Chromatic Ribbon)
    if (canvas1) {
      drawComposingOrb(canvas1, elapsed);
    }

    // 2: Solving Orb (Harmonic Pulse)
    if (canvas2) {
      drawSolvingOrb(canvas2, elapsed);
    }

    orbAnimReq = requestAnimationFrame(drawFrame);
  };

  orbAnimReq = requestAnimationFrame(drawFrame);
}

function drawBreathingOrb(canvas: HTMLCanvasElement, t: number): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const cx = w / 2;
  const cy = h / 2;
  const pulse = 0.5 + 0.5 * Math.sin(t * 2.4);

  const grad = ctx.createRadialGradient(cx, cy, 2, cx, cy, 18 + pulse * 4);
  grad.addColorStop(0, 'rgba(56, 189, 248, 0.95)');
  grad.addColorStop(0.5, 'rgba(129, 140, 248, 0.6)');
  grad.addColorStop(1, 'rgba(168, 85, 247, 0)');

  ctx.fillStyle = grad;
  ctx.beginPath();
  ctx.arc(cx, cy, 20 + pulse * 3, 0, Math.PI * 2);
  ctx.fill();

  ctx.fillStyle = '#ffffff';
  ctx.beginPath();
  ctx.arc(cx, cy, 3 + pulse, 0, Math.PI * 2);
  ctx.fill();
}

function drawComposingOrb(canvas: HTMLCanvasElement, t: number): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const cx = w / 2;
  const cy = h / 2;

  for (let i = 0; i < 3; i++) {
    const angle = t * 3.0 + (i * Math.PI * 2) / 3;
    const r = 10 + Math.sin(t * 4.0 + i) * 3;
    const px = cx + Math.cos(angle) * r;
    const py = cy + Math.sin(angle) * r;

    ctx.fillStyle = i === 0 ? 'rgba(56, 189, 248, 0.85)' : i === 1 ? 'rgba(168, 85, 247, 0.85)' : 'rgba(236, 72, 153, 0.85)';
    ctx.beginPath();
    ctx.arc(px, py, 4, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawSolvingOrb(canvas: HTMLCanvasElement, t: number): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const cx = w / 2;
  const cy = h / 2;

  ctx.strokeStyle = '#38bdf8';
  ctx.lineWidth = 2;
  ctx.beginPath();
  const ringR = 12 + Math.sin(t * 3.5) * 3;
  ctx.arc(cx, cy, ringR, 0, Math.PI * 2);
  ctx.stroke();

  ctx.fillStyle = '#60a5fa';
  ctx.beginPath();
  ctx.arc(cx, cy, 4, 0, Math.PI * 2);
  ctx.fill();
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
  updateScaleUI();
  updateOrbCardsUI();
  updateFontCardsUI();

  setSwitchChecked('toggle-app-line-numbers', currentSettings.show_line_numbers);
  setSwitchChecked('toggle-app-inline-code', currentSettings.highlight_inline_code);
  setSwitchChecked('toggle-app-compact', currentSettings.compact_mode);
  setSwitchChecked('toggle-app-animations', currentSettings.smooth_animations);

  applyLiveScale(currentSettings.selected_ui_scale);
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
