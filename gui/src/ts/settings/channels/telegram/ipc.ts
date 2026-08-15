// Telegram Settings IPC Wrappers

import { invokeIpc } from '../../../shared/ipc.js';
import type { SaveTelegramPayload, TelegramState } from './types.js';

export async function getTelegramStateIpc(): Promise<TelegramState | null> {
  return await invokeIpc<TelegramState>('get_telegram_state');
}

export async function checkTelegramPolicyCoverageIpc(
  workspaceDir: string
): Promise<boolean> {
  const res = await invokeIpc<boolean>('check_telegram_policy_coverage', {
    workspaceDir,
  });
  return res ?? false;
}

export async function pickTelegramWorkspaceDialogIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('pick_telegram_workspace_dialog');
}

export async function testTelegramChannelConnectionIpc(
  botToken: string
): Promise<string> {
  const res = await invokeIpc<string>('test_telegram_channel_connection', {
    botToken,
  });
  return res || '';
}

export async function saveTelegramChannelConfigIpc(
  payload: SaveTelegramPayload
): Promise<void> {
  await invokeIpc('save_telegram_channel_config', { payload });
}
