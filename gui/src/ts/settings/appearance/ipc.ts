// Appearance Settings IPC Wrappers

import { invokeIpc } from '../../shared/ipc.js';
import type { AppearanceSettings } from './types.js';

export async function getAppearanceSettingsIpc(): Promise<AppearanceSettings> {
  const res = await invokeIpc<AppearanceSettings>('get_appearance_settings');
  return (
    res || {
      selected_theme: 0,
      selected_ui_scale: 1,
      compact_mode: false,
      smooth_animations: true,
      selected_thinking_orb: 1,
      selected_ui_font: 0,
      selected_assistant_font: 0,
      selected_code_font: 0,
      cursor_blink_enabled: true,
    }
  );
}

export async function saveAppearanceSettingsIpc(settings: AppearanceSettings): Promise<void> {
  await invokeIpc('save_appearance_settings', { settings });
}
