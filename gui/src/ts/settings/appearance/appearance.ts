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
  cursor_blink_enabled: true,
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

  setupThemeCards();
  setupScaleSelector();
  setupToggleSwitches();
  setupOrbCards();
  setupFontSelectors();
  startLiveOrbPreviews();
  syncAppearanceUI();
}

/**
 * Binds theme selection cards.
 */
function setupThemeCards(): void {
  const cards = document.querySelectorAll<HTMLElement>('.theme-preview-card');
  cards.forEach((card) => {
    card.addEventListener('click', async () => {
      const idx = parseInt(card.dataset.index || '0', 10);
      currentSettings.selected_theme = idx;
      updateThemeCardsUI();
      applyLiveTheme(idx);
      await persist();
    });
  });
}

function updateThemeCardsUI(): void {
  const cards = document.querySelectorAll<HTMLElement>('.theme-preview-card');
  cards.forEach((card) => {
    const idx = parseInt(card.dataset.index || '0', 10);
    card.classList.toggle('selected', idx === currentSettings.selected_theme);
  });
}

function applyLiveTheme(themeIndex: number): void {
  const root = document.documentElement;
  if (themeIndex === 1) {
    // Midnight OLED
    root.style.setProperty('--window-background', '#000000');
    root.style.setProperty('--titlebar-background', '#0a0a0a');
  } else if (themeIndex === 2) {
    // GitHub Dark
    root.style.setProperty('--window-background', '#0d1117');
    root.style.setProperty('--titlebar-background', '#161b22');
  } else if (themeIndex === 3) {
    // Tokyo Night
    root.style.setProperty('--window-background', '#1a1b26');
    root.style.setProperty('--titlebar-background', '#16161e');
  } else {
    // Operon Dark (Default)
    root.style.setProperty('--window-background', '#181818');
    root.style.setProperty('--titlebar-background', '#191919');
  }
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
  bindSwitch('toggle-app-compact', currentSettings.compact_mode, async (val) => {
    currentSettings.compact_mode = val;
    await persist();
  });

  bindSwitch('toggle-app-animations', currentSettings.smooth_animations, async (val) => {
    currentSettings.smooth_animations = val;
    await persist();
  });

  bindSwitch('toggle-app-cursor-blink', currentSettings.cursor_blink_enabled, async (val) => {
    currentSettings.cursor_blink_enabled = val;
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
 * Binds typography font selectors (UI Font, Assistant Font, Code Font).
 */
function setupFontSelectors(): void {
  // UI Font
  bindSegmentedChoice('.seg-choice-ui-font', currentSettings.selected_ui_font, async (idx) => {
    currentSettings.selected_ui_font = idx;
    applyLiveFonts();
    await persist();
  });

  // Assistant Font
  bindSegmentedChoice('.seg-choice-assistant-font', currentSettings.selected_assistant_font, async (idx) => {
    currentSettings.selected_assistant_font = idx;
    applyLiveFonts();
    await persist();
  });

  // Code Font
  bindSegmentedChoice('.seg-choice-code-font', currentSettings.selected_code_font, async (idx) => {
    currentSettings.selected_code_font = idx;
    applyLiveFonts();
    await persist();
  });
}

function applyLiveFonts(): void {
  const root = document.documentElement;

  // UI Font
  const uiFonts = ['"Open Sans", sans-serif', '"Inter", sans-serif', '"Roboto", sans-serif'];
  root.style.setProperty('--font-family', uiFonts[currentSettings.selected_ui_font] || uiFonts[0]);

  // Assistant Font
  const astFonts = ['"Literata", serif', '"Georgia", serif', '"Merriweather", serif'];
  root.style.setProperty('--assistant-font-family', astFonts[currentSettings.selected_assistant_font] || astFonts[0]);

  // Code Font
  const codeFonts = ['"Kode Mono", monospace', '"Fira Code", monospace', '"JetBrains Mono", monospace'];
  root.style.setProperty('--mono-font-family', codeFonts[currentSettings.selected_code_font] || codeFonts[0]);
}

function bindSegmentedChoice(selector: string, initialIndex: number, onChange: (idx: number) => Promise<void>): void {
  const buttons = document.querySelectorAll<HTMLButtonElement>(selector);
  buttons.forEach((btn) => {
    const idx = parseInt(btn.dataset.index || '0', 10);
    btn.classList.toggle('active', idx === initialIndex);

    btn.addEventListener('click', async () => {
      buttons.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');
      await onChange(idx);
    });
  });
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
  updateThemeCardsUI();
  updateScaleUI();
  updateOrbCardsUI();

  setSwitchChecked('toggle-app-compact', currentSettings.compact_mode);
  setSwitchChecked('toggle-app-animations', currentSettings.smooth_animations);
  setSwitchChecked('toggle-app-cursor-blink', currentSettings.cursor_blink_enabled);

  applyLiveTheme(currentSettings.selected_theme);
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
