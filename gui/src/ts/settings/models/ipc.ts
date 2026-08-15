// Models Settings IPC Wrappers

import { invokeIpc } from '../../shared/ipc.js';
import type { ProviderSetupDetails, ProviderSummary, SaveProviderRequest } from './types.js';

export async function getProvidersListIpc(): Promise<ProviderSummary[]> {
  const res = await invokeIpc<ProviderSummary[]>('get_providers_list');
  return res || [];
}

export async function getProviderSetupDetailsIpc(
  providerId: string
): Promise<ProviderSetupDetails | null> {
  return await invokeIpc<ProviderSetupDetails>('get_provider_setup_details', {
    providerId,
  });
}

export async function discoverProviderModelsIpc(
  providerId: string,
  apiBase: string,
  apiKey: string
): Promise<string[]> {
  const res = await invokeIpc<string[]>('discover_provider_models', {
    providerId,
    apiBase,
    apiKey,
  });
  return res || [];
}

export async function saveProviderConfigIpc(request: SaveProviderRequest): Promise<void> {
  await invokeIpc('save_provider_config', { request });
}
