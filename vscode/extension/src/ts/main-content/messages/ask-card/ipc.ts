// IPC caller for responding to interactive Ask Question clarifying prompts

import { invokeIpc } from '../../../shared/ipc.js';

/**
 * Sends the user's response (selected MCQ option or custom text) to the active session runner.
 */
export async function respondToAskIpc(id: string, answer: string): Promise<void> {
  await invokeIpc('respond_to_ask', { id, answer });
}
