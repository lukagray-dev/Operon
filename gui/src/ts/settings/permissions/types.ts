// Permissions Settings TypeScript Interfaces matching Slint 1:1

export interface AllowedDirectories {
  directories: string[];
  workspace_directory: string;
}

export interface PermissionItem {
  key: string;
  label: string;
  subtitle: string;
  mode: string; // "allow" | "ask" | "deny"
  base_mode: string;
  is_explicit: boolean;
  kind: string; // "group" | "tool"
  group_key: string;
  is_expanded: boolean;
  has_tools: boolean;
}

export interface UpdatePermissionRequest {
  scope: string; // "owner" | "external"
  directory?: string;
  key: string;
  kind: string;
  mode: string; // "allow" | "ask" | "deny"
}
