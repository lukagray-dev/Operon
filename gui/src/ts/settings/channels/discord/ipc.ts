// Discord Settings IPC Wrappers

import { invokeIpc } from '../../../shared/ipc.js';
import type { DiscordState, SaveDiscordPayload } from './types.js';

export async function getDiscordStateIpc(): Promise<DiscordState | null> {
  return await invokeIpc<DiscordState>('get_discord_state');
}

export async function checkDiscordPolicyCoverageIpc(
  workspaceDir: string
): Promise<boolean> {
  const res = await invokeIpc<boolean>('check_discord_policy_coverage', {
    workspaceDir,
  });
  return res ?? false;
}

export async function pickDiscordWorkspaceDialogIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('pick_discord_workspace_dialog');
}

export async function testDiscordChannelConnectionIpc(
  botToken: string
): Promise<string> {
  const res = await invokeIpc<string>('test_discord_channel_connection', {
    botToken,
  });
  return res || '';
}

export async function saveDiscordChannelConfigIpc(
  payload: SaveDiscordPayload
): Promise<void> {
  await invokeIpc('save_discord_channel_config', { payload });
}

