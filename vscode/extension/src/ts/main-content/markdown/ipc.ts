// Markdown IPC Bridge for VS Code

import { invokeIpc } from '../../shared/ipc.js';

export async function renderMarkdownIpc(markdown: string): Promise<string> {
  if (!markdown || markdown.trim().length === 0) {
    return '';
  }

  try {
    const html = await invokeIpc<string>('render_markdown', { markdown });
    return html || '';
  } catch (err) {
    console.error('[Markdown IPC] Error compiling markdown:', err);
    return `<p>${escapeHtml(markdown)}</p>`;
  }
}

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

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
