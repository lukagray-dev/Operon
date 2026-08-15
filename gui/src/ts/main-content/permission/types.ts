// Types for interactive tool permission approval requests

export interface PendingPermission {
  id: string;
  tool: string;
  path: string | null;
  reason: string;
  args_json: string;
  displayAction: string;
  displayTarget: string;
}
