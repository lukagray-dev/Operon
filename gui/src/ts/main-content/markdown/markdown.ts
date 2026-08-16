// Ultra-Smooth Live Markdown Stream Renderer & DOM Post-Processor
//
// Features:
// 1. High-speed pulldown-cmark compilation via Tauri IPC.
// 2. 60 FPS RAF-batched streaming renderer that updates DOM in real-time as tokens arrive.
// 3. GitHub-style code cards with language badges and animated SVG copy buttons (no emojis!).
// 4. Secure external hyperlink interceptor opening links safely in user's default browser.
// 5. Responsive table containers with smooth horizontal scrolling.

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
  selected_thinking_orb: 1,
  selected_ui_font: 0,
  selected_assistant_font: 0,
  selected_code_font: 0,
  code_block_theme: 0,
  show_line_numbers: true,
  highlight_inline_code: true,
  table_theme: 0,
};

export function applyGlobalFontsAndTheme(settings: AppearanceSettings): void {
  const root = document.documentElement;

  // 1. UI Font
  const uiFonts = [
    "'Open Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
    "'Roboto', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
  ];
  root.style.setProperty('--font-family', uiFonts[settings.selected_ui_font] || uiFonts[0]);

  // 2. Assistant Message Font
  const astFonts = [
    "'Literata', Georgia, serif",
    "'Lora', Georgia, serif",
    "'Merriweather', Georgia, serif",
  ];
  root.style.setProperty('--assistant-font-family', astFonts[settings.selected_assistant_font] || astFonts[0]);

  // 3. Monospace Code Font
  const codeFonts = [
    "'Kode Mono', monospace",
    "'JetBrains Mono', monospace",
    "'Fira Code', monospace",
  ];
  root.style.setProperty('--mono-font-family', codeFonts[settings.selected_code_font] || codeFonts[0]);

  // 4. UI Scale
  const scales = [0.8, 1.0, 1.2, 1.4, 1.6];
  const scale = scales[settings.selected_ui_scale] ?? 1.0;
  root.style.setProperty('--ui-scale', String(scale));
}

// Initial load & live event listener
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

/**
 * State descriptor for an active streaming markdown element.
 */
interface StreamElementState {
  element: HTMLElement;
  latestText: string;
  inFlight: boolean;
  onFrameCallback?: () => void;
}

class LiveMarkdownStreamManager {
  private activeStreams: Map<HTMLElement, StreamElementState> = new Map();
  private rafScheduled = false;

  /**
   * Queues an incoming streaming text delta for live markdown rendering.
   *
   * As tokens arrive rapidly from the LLM, this method batches DOM updates using
   * `requestAnimationFrame` and ensures at most one IPC compilation is in-flight
   * per element at a time, eliminating backpressure, race conditions, and UI flicker.
   *
   * @param element - The container DOM element (.assistant-message-body.markdown-body).
   * @param fullText - The accumulated markdown text for the active message block.
   * @param onFrameComplete - Optional callback invoked after DOM update (e.g. smartAutoScroll).
   */
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

  /**
   * Finalizes a stream for a given element, ensuring the full markdown is rendered
   * and all interactive widgets (code copy buttons, links) are attached.
   */
  public async finalizeStream(element: HTMLElement, fullText: string): Promise<void> {
    this.activeStreams.delete(element);
    const html = await renderMarkdownIpc(fullText);
    element.innerHTML = html;
    postProcessMarkdownElement(element);
  }

  /**
   * Cleans up tracking for an element when it is removed or the session resets.
   */
  public cleanupElement(element: HTMLElement): void {
    this.activeStreams.delete(element);
  }

  /**
   * Cleans up all active streaming states.
   */
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

/**
 * Singleton instance of the live streaming markdown coordinator.
 */
export const liveMarkdownRenderer = new LiveMarkdownStreamManager();

/**
 * Standard asynchronous markdown compiler for static or batch rendering.
 */
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

/**
 * Post-processes rendered HTML container:
 * 1. Highlights all `<pre><code>` blocks via highlight.js.
 * 2. Wraps `<pre>` in styled Code Cards with language badge and copy button.
 * 3. Applies line numbers and code block theme.
 * 4. Tokenizes and highlights inline `<code>` tags.
 * 5. Applies table appearance theme.
 * 6. Renders LaTeX formulas with KaTeX.
 * 7. Intercepts all `<a>` tags to open external URLs securely via Tauri IPC.
 *
 * @param container - The DOM element containing rendered Markdown.
 * @param options - Customization flags.
 */
export function postProcessMarkdownElement(
  container: HTMLElement,
  options: RenderMarkdownOptions = {}
): void {
  const enhanceCode = options.enhanceCodeBlocks ?? true;
  const highlightSyntax = options.highlightSyntax ?? true;
  const renderMath = options.renderMath ?? true;
  const interceptLinks = options.interceptLinks ?? true;

  // 1. Highlight Syntax with highlight.js
  if (highlightSyntax) {
    highlightCodeBlocks(container);
  }

  // 2. Enhance Code Blocks with Language Header, Theme & Line Numbers
  if (enhanceCode) {
    enhanceCodeBlocks(container);
  }

  // 3. Enhance Inline Code tags
  enhanceInlineCode(container);

  // 4. Enhance Tables with active theme
  enhanceTables(container);

  // 5. Render LaTeX Math with KaTeX
  if (renderMath) {
    renderMathFormulas(container);
  }

  // 6. Intercept External Links to open in default browser
  if (interceptLinks) {
    interceptExternalLinks(container);
  }
}

/**
 * Applies syntax highlighting to all `<pre><code>` blocks using highlight.js.
 */
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

/**
 * Wraps `<pre>` elements with a clean header containing the language badge,
 * an interactive SVG copy button, theme classes, and optional line numbers.
 */
function enhanceCodeBlocks(container: HTMLElement): void {
  const preElements = container.querySelectorAll<HTMLPreElement>('pre');
  const themeClass = getCodeThemeClass(cachedAppearance.code_block_theme);
  const showLines = cachedAppearance.show_line_numbers;

  preElements.forEach((pre) => {
    const codeEl = pre.querySelector('code');
    const rawCode = codeEl ? codeEl.textContent || '' : pre.textContent || '';

    // If already wrapped, update classes & line numbers
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

    // Extract language identifier from class (e.g. language-rust, language-typescript)
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

    // Build the outer wrapper card with active theme
    const wrapper = document.createElement('div');
    wrapper.className = `code-block-wrapper ${themeClass}`;

    // Build the header bar
    const header = document.createElement('div');
    header.className = 'code-block-header';

    // Language label
    const langLabel = document.createElement('span');
    langLabel.className = 'code-block-lang';
    langLabel.textContent = lang;

    // Copy button
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

    // Copy to clipboard handler with feedback animation
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

    // Build code body container
    const body = document.createElement('div');
    body.className = 'code-block-body';

    if (showLines) {
      const gutter = createLineNumbersElement(rawCode);
      body.appendChild(gutter);
    }

    // Structure DOM: Insert wrapper where pre was
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

/**
 * Enhances inline code tags with syntax keyword/symbol tokenization or plain pill styling.
 */
function enhanceInlineCode(container: HTMLElement): void {
  const inlineCodeElements = container.querySelectorAll<HTMLElement>('code:not(pre code)');
  const isHighlighted = cachedAppearance.highlight_inline_code;

  inlineCodeElements.forEach((code) => {
    if (isHighlighted) {
      code.classList.remove('inline-code-plain');
      code.classList.add('inline-code-highlighted');

      // If text not tokenized yet
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

  // Escape HTML
  const escaped = raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');

  // String literals
  if (/^(&quot;|&#039;|`).*(&quot;|&#039;|`)$/.test(escaped.trim())) {
    return `<span class="hl-str">${escaped}</span>`;
  }

  // Numeric literals
  if (/^(\d+(\.\d+)?|0x[0-9a-fA-F]+)$/.test(trimmed)) {
    return `<span class="hl-num">${escaped}</span>`;
  }

  // Keywords
  const keywords = new Set([
    'fn', 'let', 'const', 'var', 'function', 'import', 'export', 'return', 'async', 'await',
    'struct', 'enum', 'class', 'trait', 'impl', 'def', 'pub', 'type', 'interface', 'match',
    'if', 'else', 'while', 'for', 'in', 'loop', 'break', 'continue', 'true', 'false', 'null',
    'None', 'Some', 'Ok', 'Err', 'self', 'Self', 'mut', 'use', 'from', 'as'
  ]);

  if (keywords.has(trimmed)) {
    return `<span class="hl-kw">${escaped}</span>`;
  }

  // Function call pattern e.g. foo() or foo(...)
  if (/^[a-zA-Z_][a-zA-Z0-9_]*\(.*\)$/.test(trimmed)) {
    return escaped.replace(/^([a-zA-Z_][a-zA-Z0-9_]*)/, '<span class="hl-fn">$1</span>');
  }

  return escaped;
}

/**
 * Enhances tables with active theme styling.
 */
function enhanceTables(container: HTMLElement): void {
  const tables = container.querySelectorAll<HTMLTableElement>('table');
  const themeClass = getTableThemeClass(cachedAppearance.table_theme);

  tables.forEach((table) => {
    table.className = `markdown-table ${themeClass}`;
  });
}

/**
 * Renders mathematical expressions wrapped in `.math.math-inline` and `.math.math-display`
 * using the locally bundled KaTeX engine.
 */
function renderMathFormulas(container: HTMLElement): void {
  if (!window.katex) return;

  // 1. Inline Math: $...$
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

  // 2. Display Math: $$...$$
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

/**
 * Intercepts hyperlinks so clicking them delegates to Tauri's safe external URL opener.
 */
function interceptExternalLinks(container: HTMLElement): void {
  const links = container.querySelectorAll<HTMLAnchorElement>('a[href]:not(.handled-link)');

  links.forEach((link) => {
    link.classList.add('handled-link');
    const href = link.getAttribute('href');
    if (!href) return;

    // Only intercept external or web URLs (http, https, mailto)
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
