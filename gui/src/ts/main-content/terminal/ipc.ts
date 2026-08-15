// Terminal IPC Bridge
//
// Type-safe IPC invocations for creating, writing to, resizing, and closing
// pseudo-terminal (PTY) shell processes on the Rust backend.

import { invokeIpc, listenIpcEvent } from '../../shared/ipc.js';
import type { TerminalClosedPayload, TerminalOutputPayload } from './types.js';

/**
 * Spawns a new pseudo-terminal process with the specified ID and dimensions.
 *
 * @param id - Unique tab identifier (e.g. "term_123456").
 * @param cols - Initial width in character columns.
 * @param rows - Initial height in character rows.
 * @param workdir - Optional custom start directory.
 */
export async function createTerminalIpc(
  id: string,
  cols: number,
  rows: number,
  workdir?: string | null
): Promise<void> {
  await invokeIpc<void>('create_terminal', { id, cols, rows, workdir });
}

/**
 * Writes user keystrokes or command strings into the running PTY stdin.
 *
 * @param id - Unique tab identifier.
 * @param input - Character stream or keystroke bytes.
 */
export async function writeTerminalIpc(id: string, input: string): Promise<void> {
  await invokeIpc<void>('write_terminal', { id, input });
}

/**
 * Resizes the PTY character grid dimensions.
 *
 * @param id - Unique tab identifier.
 * @param cols - New column count.
 * @param rows - New row count.
 */
export async function resizeTerminalIpc(id: string, cols: number, rows: number): Promise<void> {
  await invokeIpc<void>('resize_terminal', { id, cols, rows });
}

/**
 * Terminates and closes a running terminal process.
 *
 * @param id - Unique tab identifier.
 */
export async function closeTerminalIpc(id: string): Promise<void> {
  await invokeIpc<void>('close_terminal', { id });
}

/**
 * Retrieves the resolved default workspace or active project directory from the backend.
 */
export async function getTerminalDefaultWorkdirIpc(): Promise<string> {
  const res = await invokeIpc<string>('get_terminal_default_workdir');
  return res || '';
}

/**
 * Subscribes to asynchronous output chunks emitted by the backend PTY reader thread.
 *
 * @param callback - Event listener receiving `{ id, data }`.
 * @returns Unlisten function to cleanly detach the listener.
 */
export async function listenTerminalOutput(
  callback: (payload: TerminalOutputPayload) => void
): Promise<() => void> {
  return await listenIpcEvent<TerminalOutputPayload>('terminal-output', callback);
}

/**
 * Subscribes to process termination events emitted when a PTY process exits.
 *
 * @param callback - Event listener receiving `{ id }`.
 * @returns Unlisten function to cleanly detach the listener.
 */
export async function listenTerminalClosed(
  callback: (payload: TerminalClosedPayload) => void
): Promise<() => void> {
  return await listenIpcEvent<TerminalClosedPayload>('terminal-closed', callback);
}
