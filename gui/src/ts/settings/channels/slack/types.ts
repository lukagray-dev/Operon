// Slack Channel Settings Type Definitions

export interface SlackStateDto {
  connection_status: string;
  bot_token: string;
  app_token: string;
  owner_user_id: string;
  allowlist: string[];
  workspace_dir: string;
  resolved_workspace_placeholder: string;
  is_policy_covered: boolean;
}

export interface SaveSlackPayloadDto {
  bot_token: string;
  app_token: string;
  owner_user_id: string;
  allowlist: string[];
  workspace_dir: string;
}

