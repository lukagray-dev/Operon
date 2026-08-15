// Markdown Rendering & Streaming Types
//
// Defines interfaces for compiling and post-processing Markdown content
// in assistant responses.

export interface RenderMarkdownOptions {
  /** If true, enhances code blocks with copy buttons and language badges. Defaults to true. */
  enhanceCodeBlocks?: boolean;
  /** If true, intercepts all hyperlinks to open externally in default browser. Defaults to true. */
  interceptLinks?: boolean;
  /** If true, wraps GFM tables inside a scrollable container. Defaults to true. */
  wrapTables?: boolean;
  /** If true, compiles LaTeX math using KaTeX. Defaults to true. */
  renderMath?: boolean;
  /** If true, highlights code blocks using highlight.js. Defaults to true. */
  highlightSyntax?: boolean;
}

export interface StreamRenderTask {
  element: HTMLElement;
  text: string;
  inFlight: boolean;
  needsUpdate: boolean;
}
