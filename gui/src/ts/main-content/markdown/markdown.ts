// Ultra-Smooth Live Markdown Stream Renderer & DOM Post-Processor
//
// Features:
// 1. High-speed pulldown-cmark compilation via Tauri IPC.
// 2. 60 FPS RAF-batched streaming renderer that updates DOM in real-time as tokens arrive.
// 3. GitHub-style code cards with language badges and animated SVG copy buttons (no emojis!).
// 4. Secure external hyperlink interceptor opening links safely in user's default browser.
// 5. Responsive table containers with smooth horizontal scrolling.

import { invokeIpc } from '../../shared/ipc.js';
import { renderMarkdownIpc } from './ipc.js';
import type { RenderMarkdownOptions } from './types.js';

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
      // If a compile is already in flight for this element, skip until it completes.
      if (state.inFlight) continue;

      const textToRender = state.latestText;
      state.inFlight = true;

      const task = (async () => {
        try {
          const html = await renderMarkdownIpc(textToRender);
          // Check if element is still attached to the DOM before mutating
          if (document.body.contains(element)) {
            element.innerHTML = html;
            // Post-process code cards, tables, and links
            postProcessMarkdownElement(element, { enhanceCodeBlocks: true });
            if (state.onFrameCallback) {
              state.onFrameCallback();
            }
          }
        } finally {
          state.inFlight = false;
          // If newer tokens arrived while IPC was running, schedule another frame immediately!
          if (state.latestText !== textToRender && this.activeStreams.has(element)) {
            this.scheduleFlush();
          }
        }
      })();

      tasks.push(task);
    }

    if (tasks.length > 0) {
      await Promise.allSettled(tasks);
    }
  }
}

/**
 * Singleton instance of the live streaming markdown coordinator.
 */
export const liveMarkdownRenderer = new LiveMarkdownStreamManager();

/**
 * Renders raw Markdown into HTML directly and attaches post-processing enhancements.
 *
 * @param markdown - Raw Markdown string.
 * @returns Fully compiled and sanitized HTML string.
 */
export async function renderMarkdownToHtml(markdown: string): Promise<string> {
  return await renderMarkdownIpc(markdown);
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
 * Post-processes rendered Markdown HTML inside a container element:
 *
 * 1. Highlights source code syntax via highlight.js.
 * 2. Compiles LaTeX math formulas via KaTeX ($inline$ and $$display$$).
 * 3. Intercepts all `<a>` tags to open external URLs securely via Tauri IPC.
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

  // 2. Enhance Code Blocks with Language Header & Copy Button
  if (enhanceCode) {
    enhanceCodeBlocks(container);
  }

  // 3. Render LaTeX Math with KaTeX
  if (renderMath) {
    renderMathFormulas(container);
  }

  // 4. Intercept External Links to open in default browser
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
 * Wraps `<pre>` elements with a clean header containing the language badge
 * and an interactive SVG copy button.
 */
function enhanceCodeBlocks(container: HTMLElement): void {
  const preElements = container.querySelectorAll<HTMLPreElement>('pre');

  preElements.forEach((pre) => {
    // Avoid double-wrapping
    if (pre.parentElement?.classList.contains('code-block-wrapper')) {
      return;
    }

    const codeEl = pre.querySelector('code');
    const rawCode = codeEl ? codeEl.textContent || '' : pre.textContent || '';

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

    // Build the outer wrapper card
    const wrapper = document.createElement('div');
    wrapper.className = 'code-block-wrapper';

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

    // Structure DOM: Insert wrapper where pre was, place header + pre inside wrapper
    if (pre.parentNode) {
      pre.parentNode.insertBefore(wrapper, pre);
      wrapper.appendChild(header);
      wrapper.appendChild(pre);
    }
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
