// Terminal Panel Types & Interfaces
//
// Defines tab state descriptors, payload types for Tauri IPC streaming,
// and configuration parameters for pseudo-terminal emulation.

export interface XtermTerminal {
  open(element: HTMLElement): void;
  write(data: string): void;
  loadAddon(addon: unknown): void;
  onData(callback: (data: string) => void): { dispose(): void };
  onResize(callback: (dims: { cols: number; rows: number }) => void): { dispose(): void };
  focus(): void;
  blur(): void;
  dispose(): void;
  cols: number;
  rows: number;
}

export interface XtermFitAddon {
  fit(): void;
  proposeDimensions(): { cols: number; rows: number } | undefined;
}

export interface TerminalTab {
  id: string;
  name: string;
  term: XtermTerminal;
  fitAddon: XtermFitAddon;
  wrapperEl: HTMLElement;
  tabEl: HTMLElement;
  workdir?: string | null;
}

export interface TerminalOutputPayload {
  id: string;
  data: string;
}

export interface TerminalClosedPayload {
  id: string;
}
