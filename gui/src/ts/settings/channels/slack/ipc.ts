// Slack Settings IPC callers

import { invokeIpc } from '../../../shared/ipc.js';
import type { SaveSlackPayloadDto, SlackStateDto } from './types.js';

export async function getSlackStateIpc(): Promise<SlackStateDto> {
  const res = await invokeIpc<SlackStateDto>('get_slack_state');
  return (
    res || {
      connection_status: 'Disconnected',
      bot_token: '',
      app_token: '',
      owner_user_id: '',
      allowlist: [],
      workspace_dir: '',
      resolved_workspace_placeholder: '~/.operon/workspace',
      is_policy_covered: false,
    }
  );
}

export async function checkSlackPolicyCoverageIpc(workspaceDir: string): Promise<boolean> {
  const res = await invokeIpc<boolean>('check_slack_policy_coverage', { workspaceDir });
  return res ?? false;
}

export async function pickSlackWorkspaceDialogIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('pick_slack_workspace_dialog');
}

export async function testSlackChannelConnectionIpc(botToken: string): Promise<string> {
  const res = await invokeIpc<string>('test_slack_channel_connection', { botToken });
  return res || '';
}

export async function saveSlackChannelConfigIpc(payload: SaveSlackPayloadDto): Promise<void> {
  await invokeIpc('save_slack_channel_config', { payload });
}

