// Main Content Input Panel IPC Callers for VS Code

import { invokeIpc } from '../../shared/ipc.js';
import type { ContextUsage, ModelOption, PendingAttachment } from './types.js';

export async function getAvailableModelsIpc(): Promise<ModelOption[]> {
  const res = await invokeIpc<ModelOption[]>('get_available_models');
  return res || [];
}

export async function selectModelIpc(modelId: string, reasoning?: string, contextWindow?: number): Promise<void> {
  await invokeIpc('select_model', { modelId, reasoning, contextWindow });
}

export async function toggleAutoApproveIpc(enabled: boolean): Promise<boolean> {
  const res = await invokeIpc<boolean>('toggle_auto_approve', { enabled });
  return res ?? enabled;
}

export async function pickAttachmentsIpc(): Promise<PendingAttachment[]> {
  const res = await invokeIpc<PendingAttachment[]>('pick_attachments_dialog');
  return res || [];
}

export async function getContextUsageIpc(sessionId?: string): Promise<ContextUsage> {
  const res = await invokeIpc<ContextUsage>('get_context_window_info', { sessionId });
  return res || { tokens_used: 0, tokens_total: 128000, percentage: 0, formatted: '0 / 128k' };
}
