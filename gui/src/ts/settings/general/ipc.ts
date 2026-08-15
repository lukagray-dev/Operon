// General Settings IPC Wrappers

import { invokeIpc } from '../../shared/ipc.js';
import type { GeneralSettings } from './types.js';

export async function getGeneralSettingsIpc(): Promise<GeneralSettings> {
  const res = await invokeIpc<GeneralSettings>('get_general_settings');
  return (
    res || {
      autostart_enabled: false,
      minimize_to_tray_enabled: false,
      start_minimized: false,
      close_button_action: 0,
      global_auto_approve_default: false,
      auto_scroll_stream: true,
      notify_on_permission_request: true,
      notify_on_response_complete: false,
      auto_collapse_reasoning_tools: false,
      auto_update_checks: true,
      telemetry_enabled: false,
    }
  );
}

export async function saveGeneralSettingsIpc(settings: GeneralSettings): Promise<void> {
  await invokeIpc('save_general_settings', { settings });
}
