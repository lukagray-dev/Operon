// Channels Root IPC Wrappers

import { invokeIpc } from '../../shared/ipc.js';
import type { ChannelCard } from './types.js';

export async function getChannelsListIpc(): Promise<ChannelCard[]> {
  const res = await invokeIpc<ChannelCard[]>('get_channels_list');
  return res || [];
}
