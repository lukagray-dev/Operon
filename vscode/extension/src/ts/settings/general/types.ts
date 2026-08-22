// General Settings TypeScript Interfaces matching Slint 1:1

export interface GeneralSettings {
  autostart_enabled: boolean;
  minimize_to_tray_enabled: boolean;
  start_minimized: boolean;
  close_button_action: number; // 0 = Exit App, 1 = Minimize to Tray
  global_auto_approve_default: boolean;
  auto_scroll_stream: boolean;
  notify_on_permission_request: boolean;
  notify_on_response_complete: boolean;
  auto_collapse_reasoning_tools: boolean;
  auto_update_checks: boolean;
  telemetry_enabled: boolean;
}
