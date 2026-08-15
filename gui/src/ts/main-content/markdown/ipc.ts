// Markdown Tauri IPC Bridge
//
// Sends Markdown compilation requests to the native pulldown-cmark backend.

import { invokeIpc } from '../../shared/ipc.js';

/**
 * Invokes the backend `render_markdown` command to compile Markdown into HTML.
 *
 * @param markdown - The raw Markdown string.
 * @returns The rendered HTML string.
 */
export async function renderMarkdownIpc(markdown: string): Promise<string> {
  if (!markdown || markdown.trim().length === 0) {
    return '';
  }

  try {
    const html = await invokeIpc<string>('render_markdown', { markdown });
    return html || '';
  } catch (err) {
    console.error('[Markdown IPC] Error compiling markdown:', err);
    // Fallback: Return raw text wrapped in paragraph if IPC fails
    return `<p>${escapeHtml(markdown)}</p>`;
  }
}

/**
 * Invokes the backend `render_markdown_batch` command to compile multiple strings.
 *
 * @param texts - Array of Markdown strings.
 * @returns Array of rendered HTML strings.
 */
export async function renderMarkdownBatchIpc(texts: string[]): Promise<string[]> {
  if (!texts || texts.length === 0) {
    return [];
  }

  try {
    const htmls = await invokeIpc<string[]>('render_markdown_batch', { texts });
    return htmls || texts.map((t) => `<p>${escapeHtml(t)}</p>`);
  } catch (err) {
    console.error('[Markdown IPC] Error batch compiling markdown:', err);
    return texts.map((t) => `<p>${escapeHtml(t)}</p>`);
  }
}

/**
 * Quick HTML escaping utility for fallback error states.
 */
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
