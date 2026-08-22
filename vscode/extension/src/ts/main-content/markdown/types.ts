// Markdown Rendering & Streaming Types for VS Code

export interface RenderMarkdownOptions {
  enhanceCodeBlocks?: boolean;
  interceptLinks?: boolean;
  wrapTables?: boolean;
  renderMath?: boolean;
  highlightSyntax?: boolean;
}

export interface StreamRenderTask {
  element: HTMLElement;
  text: string;
  inFlight: boolean;
  needsUpdate: boolean;
}
