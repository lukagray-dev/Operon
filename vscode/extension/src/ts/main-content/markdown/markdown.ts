// Ultra-Smooth Live Markdown Stream Renderer & DOM Post-Processor for VS Code

import { invokeIpc, listenIpcEvent } from '../../shared/ipc.js';
import { getAppearanceSettingsIpc } from '../../settings/appearance/ipc.js';
import type { AppearanceSettings } from '../../settings/appearance/types.js';
import { renderMarkdownIpc } from './ipc.js';
import type { RenderMarkdownOptions } from './types.js';

let cachedAppearance: AppearanceSettings = {
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

export function getCachedAppearance(): AppearanceSettings {
  return cachedAppearance;
}

export function applyGlobalFontsAndTheme(settings: AppearanceSettings): void {
  const root = document.documentElement;

  const uiFonts = [
    "'Open Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    "'Roboto', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  ];
  root.style.setProperty('--font-family', uiFonts[settings.selected_ui_font] || uiFonts[0]);

  const astFonts = [
    "'Literata', Georgia, serif",
    "'Lora', Georgia, serif",
    "'Merriweather', Georgia, serif",
  ];
  root.style.setProperty('--assistant-font-family', astFonts[settings.selected_assistant_font] || astFonts[0]);

  const codeFonts = [
    "'Kode Mono', monospace",
    "'JetBrains Mono', monospace",
    "'Fira Code', monospace",
  ];
  root.style.setProperty('--mono-font-family', codeFonts[settings.selected_code_font] || codeFonts[0]);

  const scales = [0.8, 1.0, 1.2, 1.4, 1.6];
  const scale = scales[settings.selected_ui_scale] ?? 1.0;
  root.style.setProperty('--ui-scale', String(scale));
}

getAppearanceSettingsIpc()
  .then((settings) => {
    cachedAppearance = settings;
    applyGlobalFontsAndTheme(settings);
    reapplyAllMarkdownAppearance();
  })
  .catch(() => {});

listenIpcEvent<AppearanceSettings>('operon://appearance-changed', (settings) => {
  cachedAppearance = settings;
  applyGlobalFontsAndTheme(settings);
  reapplyAllMarkdownAppearance();
});

function reapplyAllMarkdownAppearance(): void {
  const containers = document.querySelectorAll<HTMLElement>('.markdown-body');
  containers.forEach((container) => {
    postProcessMarkdownElement(container);
  });
}

function getCodeThemeClass(themeIdx: number): string {
  switch (themeIdx) {
    case 1:
      return 'code-theme-oled';
    case 2:
      return 'code-theme-tokyo';
    case 3:
      return 'code-theme-monokai';
    default:
      return 'code-theme-github';
  }
}

function getTableThemeClass(themeIdx: number): string {
  switch (themeIdx) {
    case 1:
      return 'table-theme-minimal';
    case 2:
      return 'table-theme-zebra';
    case 3:
      return 'table-theme-grid';
    default:
      return 'table-theme-github';
  }
}

interface StreamElementState {
  element: HTMLElement;
  latestText: string;
  inFlight: boolean;
  onFrameCallback?: () => void;
}

class LiveMarkdownStreamManager {
  private activeStreams: Map<HTMLElement, StreamElementState> = new Map();
  private rafScheduled = false;

  public queueStreamUpdate(
    element: HTMLElement,
    fullText: string,
    onFrameComplete?: () => void
  ): void {
    let state = this.activeStreams.get(element);
    if (!state) {
      state = {
        element,
        latestText: fullText,
        inFlight: false,
        onFrameCallback: onFrameComplete,
      };
      this.activeStreams.set(element, state);
    } else {
      state.latestText = fullText;
      if (onFrameComplete) {
        state.onFrameCallback = onFrameComplete;
      }
    }

    this.scheduleFlush();
  }

  public async finalizeStream(element: HTMLElement, fullText: string): Promise<void> {
    this.activeStreams.delete(element);
    const html = await renderMarkdownIpc(fullText);
    element.innerHTML = html;
    postProcessMarkdownElement(element);
  }

  public cleanupElement(element: HTMLElement): void {
    this.activeStreams.delete(element);
  }

  public clearAll(): void {
    this.activeStreams.clear();
    this.rafScheduled = false;
  }

  private scheduleFlush(): void {
    if (this.rafScheduled) return;
    this.rafScheduled = true;

    requestAnimationFrame(() => {
      this.rafScheduled = false;
      this.flushPendingStreams();
    });
  }

  private async flushPendingStreams(): Promise<void> {
    const tasks: Promise<void>[] = [];

    for (const [element, state] of this.activeStreams.entries()) {
      if (state.inFlight) continue;

      state.inFlight = true;
      const textToRender = state.latestText;
      const callback = state.onFrameCallback;

      const task = renderMarkdownIpc(textToRender)
        .then((html) => {
          element.innerHTML = html;
          postProcessMarkdownElement(element, { renderMath: false });
          if (callback) {
            callback();
          }
        })
        .catch((err) => {
          console.error('[Markdown Stream] Compilation error:', err);
        })
        .finally(() => {
          state.inFlight = false;
        });

      tasks.push(task);
    }

    if (tasks.length > 0) {
      await Promise.all(tasks);
    }
  }
}

export const liveMarkdownRenderer = new LiveMarkdownStreamManager();

export async function renderMarkdownToHtml(markdownText: string): Promise<string> {
  return await renderMarkdownIpc(markdownText);
}

declare global {
  interface Window {
    katex?: {
      render(
        tex: string,
        element: HTMLElement,
        options?: { displayMode?: boolean; throwOnError?: boolean; errorColor?: string }
      ): void;
      renderToString(
        tex: string,
        options?: { displayMode?: boolean; throwOnError?: boolean; errorColor?: string }
      ): string;
    };
    hljs?: {
      highlightElement(element: HTMLElement): void;
      highlight(code: string, options: { language: string; ignoreIllegals?: boolean }): { value: string };
      highlightAuto(code: string): { value: string; language?: string };
    };
  }
}

export function postProcessMarkdownElement(
  container: HTMLElement,
  options: RenderMarkdownOptions = {}
): void {
  const enhanceCode = options.enhanceCodeBlocks ?? true;
  const highlightSyntax = options.highlightSyntax ?? true;
  const renderMath = options.renderMath ?? true;
  const interceptLinks = options.interceptLinks ?? true;

  if (highlightSyntax) {
    highlightCodeBlocks(container);
  }

  if (enhanceCode) {
    enhanceCodeBlocks(container);
  }

  enhanceInlineCode(container);
  enhanceTables(container);

  if (renderMath) {
    renderMathFormulas(container);
  }

  if (interceptLinks) {
    interceptExternalLinks(container);
  }
}

function highlightCodeBlocks(container: HTMLElement): void {
  if (!window.hljs) return;

  const codeBlocks = container.querySelectorAll<HTMLElement>('pre code:not(.hljs)');
  codeBlocks.forEach((block) => {
    try {
      window.hljs?.highlightElement(block);
    } catch (err) {
      console.debug('[hljs] Syntax highlight error:', err);
    }
  });
}

function enhanceCodeBlocks(container: HTMLElement): void {
  const preElements = container.querySelectorAll<HTMLPreElement>('pre');
  const themeClass = getCodeThemeClass(cachedAppearance.code_block_theme);
  const showLines = cachedAppearance.show_line_numbers;

  preElements.forEach((pre) => {
    const codeEl = pre.querySelector('code');
    const rawCode = codeEl ? codeEl.textContent || '' : pre.textContent || '';

    const existingWrapper = pre.closest('.code-block-wrapper') as HTMLElement | null;
    if (existingWrapper) {
      existingWrapper.className = `code-block-wrapper ${themeClass}`;

      let gutter = existingWrapper.querySelector('.code-line-numbers') as HTMLElement | null;
      if (showLines) {
        if (!gutter) {
          const body = existingWrapper.querySelector('.code-block-body') || pre.parentElement;
          if (body) {
            gutter = createLineNumbersElement(rawCode);
            body.insertBefore(gutter, pre);
          }
        }
      } else if (gutter) {
        gutter.remove();
      }
      return;
    }

    let lang = 'code';
    if (codeEl) {
      const classList = Array.from(codeEl.classList);
      for (const cls of classList) {
        if (cls.startsWith('language-')) {
          lang = cls.replace('language-', '').toLowerCase();
          break;
        }
      }
    }

    const wrapper = document.createElement('div');
    wrapper.className = `code-block-wrapper ${themeClass}`;

    const header = document.createElement('div');
    header.className = 'code-block-header';

    const langLabel = document.createElement('span');
    langLabel.className = 'code-block-lang';
    langLabel.textContent = lang;

    const copyBtn = document.createElement('button');
    copyBtn.className = 'code-block-copy-btn';
    copyBtn.type = 'button';
    copyBtn.title = 'Copy code';

    const copyIcon = document.createElement('span');
    copyIcon.className = 'code-block-copy-icon';

    const copyLabel = document.createElement('span');
    copyLabel.className = 'code-block-copy-label';
    copyLabel.textContent = 'Copy';

    copyBtn.appendChild(copyIcon);
    copyBtn.appendChild(copyLabel);

    copyBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        await navigator.clipboard.writeText(rawCode);
        copyBtn.classList.add('copied');
        copyLabel.textContent = 'Copied!';

        setTimeout(() => {
          copyBtn.classList.remove('copied');
          copyLabel.textContent = 'Copy';
        }, 2000);
      } catch (err) {
        console.error('[Markdown] Failed to copy code:', err);
      }
    });

    header.appendChild(langLabel);
    header.appendChild(copyBtn);

    const body = document.createElement('div');
    body.className = 'code-block-body';

    if (showLines) {
      const gutter = createLineNumbersElement(rawCode);
      body.appendChild(gutter);
    }

    if (pre.parentNode) {
      pre.parentNode.insertBefore(wrapper, pre);
      wrapper.appendChild(header);
      wrapper.appendChild(body);
      body.appendChild(pre);
    }
  });
}

function createLineNumbersElement(rawCode: string): HTMLElement {
  const gutter = document.createElement('div');
  gutter.className = 'code-line-numbers';
  gutter.setAttribute('aria-hidden', 'true');

  const lines = rawCode.replace(/\n$/, '').split('\n');
  const count = Math.max(1, lines.length);

  for (let i = 1; i <= count; i++) {
    const span = document.createElement('span');
    span.textContent = String(i);
    gutter.appendChild(span);
  }

  return gutter;
}

function enhanceInlineCode(container: HTMLElement): void {
  const inlineCodeElements = container.querySelectorAll<HTMLElement>('code:not(pre code)');
  const isHighlighted = cachedAppearance.highlight_inline_code;

  inlineCodeElements.forEach((code) => {
    if (isHighlighted) {
      code.classList.remove('inline-code-plain');
      code.classList.add('inline-code-highlighted');

      const raw = code.dataset.rawText || code.textContent || '';
      if (!code.dataset.rawText) {
        code.dataset.rawText = raw;
      }
      code.innerHTML = highlightInlineCodeContent(raw);
    } else {
      code.classList.remove('inline-code-highlighted');
      code.classList.add('inline-code-plain');
      if (code.dataset.rawText) {
        code.textContent = code.dataset.rawText;
      }
    }
  });
}

function highlightInlineCodeContent(raw: string): string {
  const trimmed = raw.trim();

  const escaped = raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');

  if (/^(&quot;|&#039;|`).*(&quot;|&#039;|`)$/.test(escaped.trim())) {
    return `<span class="hl-str">${escaped}</span>`;
  }

  if (/^(\d+(\.\d+)?|0x[0-9a-fA-F]+)$/.test(trimmed)) {
    return `<span class="hl-num">${escaped}</span>`;
  }

  const keywords = new Set([
    'fn', 'let', 'const', 'var', 'function', 'import', 'export', 'return', 'async', 'await',
    'struct', 'enum', 'class', 'trait', 'impl', 'def', 'pub', 'type', 'interface', 'match',
    'if', 'else', 'while', 'for', 'in', 'loop', 'break', 'continue', 'true', 'false', 'null',
    'None', 'Some', 'Ok', 'Err', 'self', 'Self', 'mut', 'use', 'from', 'as'
  ]);

  if (keywords.has(trimmed)) {
    return `<span class="hl-kw">${escaped}</span>`;
  }

  if (/^[a-zA-Z_][a-zA-Z0-9_]*\(.*\)$/.test(trimmed)) {
    return escaped.replace(/^([a-zA-Z_][a-zA-Z0-9_]*)/, '<span class="hl-fn">$1</span>');
  }

  return escaped;
}

function enhanceTables(container: HTMLElement): void {
  const tables = container.querySelectorAll<HTMLTableElement>('table');
  const themeClass = getTableThemeClass(cachedAppearance.table_theme);

  tables.forEach((table) => {
    table.className = `markdown-table ${themeClass}`;
  });
}

function renderMathFormulas(container: HTMLElement): void {
  if (!window.katex) return;

  const inlineSpans = container.querySelectorAll<HTMLElement>('span.math.math-inline:not(.katex-rendered)');
  inlineSpans.forEach((el) => {
    el.classList.add('katex-rendered');
    const tex = el.textContent || '';
    if (tex.trim().length > 0) {
      try {
        window.katex?.render(tex, el, { displayMode: false, throwOnError: false });
      } catch (err) {
        console.debug('[KaTeX] Inline math render error:', err);
      }
    }
  });

  const displaySpans = container.querySelectorAll<HTMLElement>('.math.math-display:not(.katex-rendered)');
  displaySpans.forEach((el) => {
    el.classList.add('katex-rendered');
    const tex = el.textContent || '';
    if (tex.trim().length > 0) {
      try {
        window.katex?.render(tex, el, { displayMode: true, throwOnError: false });
      } catch (err) {
        console.debug('[KaTeX] Display math render error:', err);
      }
    }
  });
}

function interceptExternalLinks(container: HTMLElement): void {
  const links = container.querySelectorAll<HTMLAnchorElement>('a[href]:not(.handled-link)');

  links.forEach((link) => {
    link.classList.add('handled-link');
    const href = link.getAttribute('href');
    if (!href) return;

    if (/^(https?|mailto):/i.test(href)) {
      link.setAttribute('target', '_blank');
      link.setAttribute('rel', 'noopener noreferrer');

      link.addEventListener('click', async (e) => {
        e.preventDefault();
        try {
          await invokeIpc('open_external_url', { url: href });
        } catch (err) {
          console.warn('[Markdown] Fallback to window.open for link:', href, err);
          window.open(href, '_blank');
        }
      });
    }
  });
}
