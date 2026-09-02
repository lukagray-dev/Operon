// Feishu / Lark Settings IPC Callers

import { invokeIpc } from '../../../shared/ipc.js';
import type { FeishuState, SaveFeishuPayload } from './types.js';

export async function getFeishuStateIpc(): Promise<FeishuState> {
  const res = await invokeIpc<FeishuState>('get_feishu_state');
  return (
    res || {
      connection_status: 'Disconnected',
      app_id: '',
      app_secret: '',
      domain: 'feishu',
      owner_user_id: '',
      allowlist: [],
      workspace_dir: '',
      resolved_workspace_placeholder: '',
      is_policy_covered: true,
    }
  );
}

export async function checkFeishuPolicyCoverageIpc(workspaceDir: string): Promise<boolean> {
  const res = await invokeIpc<boolean>('check_feishu_policy_coverage', { workspaceDir });
  return res ?? true;
}

export async function pickFeishuWorkspaceDialogIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('pick_feishu_workspace_dialog');
}

export async function testFeishuChannelConnectionIpc(
  appId: string,
  appSecret: string,
  domain: string
): Promise<string> {
  const res = await invokeIpc<string>('test_feishu_channel_connection', {
    appId,
    appSecret,
    domain,
  });
  return res || '';
}

export async function saveFeishuChannelConfigIpc(payload: SaveFeishuPayload): Promise<void> {
  await invokeIpc('save_feishu_channel_config', { payload });
}
