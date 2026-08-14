// Messages IPC Callers & Event Listeners

import { invokeIpc, listenIpcEvent } from '../../shared/ipc.js';
import type { PendingAttachment } from '../input/types.js';
import type { ChatMessage } from './types.js';

export async function loadSessionMessagesIpc(sessionId: string): Promise<ChatMessage[]> {
  const res = await invokeIpc<ChatMessage[]>('load_session_messages', { sessionId });
  return res || [];
}

export async function submitPromptIpc(
  sessionId: string | null,
  prompt: string,
  attachments: PendingAttachment[],
  workspacePath: string | null
): Promise<string> {
  const res = await invokeIpc<string>('submit_prompt', {
    sessionId: sessionId || null,
    prompt,
    attachments,
    workspacePath: workspacePath || null,
  });

  return res || '';
}

export async function cancelPromptIpc(): Promise<void> {
  await invokeIpc('cancel_prompt');
}

export async function listenAgentEvent(
  handler: (event: Record<string, unknown>) => void
): Promise<() => void> {
  return await listenIpcEvent<Record<string, unknown>>('agent-event', handler);
}

export async function listenAgentFinished(
  handler: (sessionId: string) => void
): Promise<() => void> {
  return await listenIpcEvent<string>('agent-finished', handler);
}

export async function listenAgentError(
  handler: (errorMessage: string) => void
): Promise<() => void> {
  return await listenIpcEvent<string>('agent-error', handler);
}
