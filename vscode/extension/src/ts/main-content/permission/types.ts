// Types for interactive tool permission approval requests in VS Code

export interface PendingPermission {
  id: string;
  tool: string;
  path: string | null;
  reason: string;
  args_json: string;
  displayAction: string;
  displayTarget: string;
}
