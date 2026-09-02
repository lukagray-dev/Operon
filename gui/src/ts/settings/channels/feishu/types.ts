// Feishu / Lark Channel Settings Types

export interface FeishuState {
  connection_status: string;
  app_id: string;
  app_secret: string;
  domain: string;
  owner_user_id: string;
  allowlist: string[];
  workspace_dir: string;
  resolved_workspace_placeholder: string;
  is_policy_covered: boolean;
}

export interface SaveFeishuPayload {
  app_id: string;
  app_secret: string;
  domain: string;
  owner_user_id: string;
  allowlist: string[];
  workspace_dir: string;
}

