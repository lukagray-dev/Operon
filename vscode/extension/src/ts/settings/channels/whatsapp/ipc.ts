// WhatsApp Settings IPC Wrappers

import { invokeIpc } from '../../../shared/ipc.js';
import type { SaveWhatsAppPayload, WhatsAppState } from './types.js';

export async function getWhatsAppStateIpc(): Promise<WhatsAppState | null> {
  return await invokeIpc<WhatsAppState>('get_whatsapp_state');
}

export async function checkWhatsAppPolicyCoverageIpc(
  workspaceDir: string
): Promise<boolean> {
  const res = await invokeIpc<boolean>('check_whatsapp_policy_coverage', {
    workspaceDir,
  });
  return res ?? false;
}

export async function pickWhatsAppWorkspaceDialogIpc(): Promise<string | null> {
  return await invokeIpc<string | null>('pick_whatsapp_workspace_dialog');
}

export async function saveWhatsAppChannelConfigIpc(
  payload: SaveWhatsAppPayload
): Promise<void> {
  await invokeIpc('save_whatsapp_channel_config', { payload });
}

export async function startWhatsAppQrPairingIpc(): Promise<string> {
  const res = await invokeIpc<string>('start_whatsapp_qr_pairing');
  return res || '';
}

export async function startWhatsAppCodePairingIpc(
  phoneNumber: string
): Promise<string> {
  const res = await invokeIpc<string>('start_whatsapp_code_pairing', {
    phoneNumber,
  });
  return res || '';
}
